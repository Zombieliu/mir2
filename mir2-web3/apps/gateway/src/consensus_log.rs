use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    FinalizedGuildNodeRegistration, GameRewardPolicy, GuildNodeAdmission,
    GuildNodeSecurityRegistry, MultiGameRewardLedger, NodeCapacityCertificate,
    RewardSettlementBatch, SuiFinalityProof, ZoneHostControlPlane, ZoneHostHeartbeat,
    ZoneHostRegistration, ZoneId, ZonePlacementLease, ZoneRebalanceMove,
};

const CONTROL_BLOCK_DOMAIN: &[u8] = b"obelisk.mir2.control-block.v1\0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlCommandEnvelope {
    pub namespace: String,
    pub idempotency_key: String,
    pub payload: Vec<u8>,
}

impl ControlCommandEnvelope {
    pub fn json(
        namespace: impl Into<String>,
        idempotency_key: impl Into<String>,
        value: &impl Serialize,
    ) -> Result<Self, String> {
        Ok(Self {
            namespace: namespace.into(),
            idempotency_key: idempotency_key.into(),
            payload: serde_json::to_vec(value)
                .map_err(|error| format!("control command JSON encode failed: {error}"))?,
        })
    }

    fn validate(&self) -> Result<(), String> {
        validate_component("control command namespace", &self.namespace)?;
        validate_component("control command idempotency key", &self.idempotency_key)?;
        if self.payload.is_empty() {
            return Err("control command payload must not be empty".to_string());
        }
        if self.payload.len() > 1024 * 1024 {
            return Err("control command payload exceeds 1 MiB".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlBlock {
    pub epoch: u64,
    pub height: u64,
    pub view: u64,
    pub parent_digest: String,
    pub proposer: String,
    pub commands: Vec<ControlCommandEnvelope>,
    pub digest: String,
}

impl ControlBlock {
    fn new(
        epoch: u64,
        height: u64,
        view: u64,
        parent_digest: String,
        proposer: String,
        commands: Vec<ControlCommandEnvelope>,
    ) -> Result<Self, String> {
        if commands.is_empty() {
            return Err("control blocks are event-driven and must contain a command".to_string());
        }
        validate_component("control block proposer", &proposer)?;
        let mut idempotency_keys = BTreeSet::new();
        for command in &commands {
            command.validate()?;
            if !idempotency_keys.insert(command.idempotency_key.as_str()) {
                return Err(format!(
                    "duplicate control command idempotency key {}",
                    command.idempotency_key
                ));
            }
        }
        let digest =
            control_block_digest(epoch, height, view, &parent_digest, &proposer, &commands)?;
        Ok(Self {
            epoch,
            height,
            view,
            parent_digest,
            proposer,
            commands,
            digest,
        })
    }

    pub fn verify_digest(&self) -> Result<(), String> {
        let expected = control_block_digest(
            self.epoch,
            self.height,
            self.view,
            &self.parent_digest,
            &self.proposer,
            &self.commands,
        )?;
        if expected != self.digest {
            return Err(format!(
                "control block digest mismatch: expected {expected}, got {}",
                self.digest
            ));
        }
        Ok(())
    }

    #[cfg(feature = "commonware-2026-2")]
    pub fn commonware_coordinates(
        &self,
    ) -> (
        commonware_consensus::types::Epoch,
        commonware_consensus::types::Height,
        commonware_consensus::types::View,
    ) {
        use commonware_consensus::types::{Epoch, Height, View};
        (
            Epoch::new(self.epoch),
            Height::new(self.height),
            View::new(self.view),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinalizedControlBlock {
    pub block: ControlBlock,
    pub signers: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ReplicatedControlCommand {
    RegisterZoneHost {
        registration: ZoneHostRegistration,
        heartbeat: ZoneHostHeartbeat,
    },
    ZoneHostHeartbeat {
        host_id: String,
        heartbeat: ZoneHostHeartbeat,
    },
    PlaceZone {
        zone_id: ZoneId,
        now_ms: u64,
    },
    BeginZoneHostDrain {
        host_id: String,
        now_ms: u64,
    },
    FinishZoneHostDrain {
        host_id: String,
    },
    AdmitGuildNode {
        admission: GuildNodeAdmission,
        now_ms: u64,
    },
    RevokeGuildNode {
        node_id: String,
    },
    SyncFinalizedGuildNode {
        registration: FinalizedGuildNodeRegistration,
        capacity_certificate: NodeCapacityCertificate,
        now_ms: u64,
    },
    RevokeFinalizedGuildNode {
        node_id: String,
        finality: SuiFinalityProof,
    },
    RegisterGameRewardPolicy {
        policy: GameRewardPolicy,
    },
    FinalizeGameRewardEpoch {
        game_id: String,
        epoch: u64,
    },
}

impl ReplicatedControlCommand {
    pub fn envelope(
        &self,
        idempotency_key: impl Into<String>,
    ) -> Result<ControlCommandEnvelope, String> {
        ControlCommandEnvelope::json("obelisk.control.v1", idempotency_key, self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectedControlEffect {
    ZoneHostRegistered(String),
    ZoneHostHeartbeatAccepted(String),
    ZonePlaced(ZonePlacementLease),
    ZoneHostDrainStarted(Vec<ZoneRebalanceMove>),
    ZoneHostDrainFinished(String),
    GuildNodeAdmitted(String),
    GuildNodeRevoked {
        node_id: String,
        existed: bool,
    },
    FinalizedGuildNodeSynced {
        node_id: String,
        sui_checkpoint: u64,
    },
    FinalizedGuildNodeRevoked {
        node_id: String,
        sui_checkpoint: u64,
        existed: bool,
    },
    RewardPolicyRegistered {
        game_id: String,
        epoch: u64,
    },
    RewardEpochFinalized(RewardSettlementBatch),
}

#[derive(Debug)]
pub struct FinalizedControlProjector {
    scheduler: Arc<ZoneHostControlPlane>,
    guild_security: Arc<GuildNodeSecurityRegistry>,
    reward_ledger: Option<Arc<Mutex<MultiGameRewardLedger>>>,
    last_height: Mutex<u64>,
}

impl FinalizedControlProjector {
    pub fn new(
        scheduler: Arc<ZoneHostControlPlane>,
        guild_security: Arc<GuildNodeSecurityRegistry>,
    ) -> Self {
        Self {
            scheduler,
            guild_security,
            reward_ledger: None,
            last_height: Mutex::new(0),
        }
    }

    pub fn with_reward_ledger(mut self, reward_ledger: Arc<Mutex<MultiGameRewardLedger>>) -> Self {
        self.reward_ledger = Some(reward_ledger);
        self
    }

    pub fn last_height(&self) -> u64 {
        self.last_height
            .lock()
            .map(|height| *height)
            .unwrap_or_default()
    }

    pub fn apply(
        &self,
        finalized: &FinalizedControlBlock,
    ) -> Result<Vec<ProjectedControlEffect>, String> {
        let mut last_height = self
            .last_height
            .lock()
            .map_err(|_| "finalized control projector mutex poisoned".to_string())?;
        let expected_height = last_height.saturating_add(1);
        if finalized.block.height != expected_height {
            return Err(format!(
                "control projector height gap: expected {expected_height}, got {}",
                finalized.block.height
            ));
        }
        let mut effects = Vec::with_capacity(finalized.block.commands.len());
        for envelope in &finalized.block.commands {
            if envelope.namespace != "obelisk.control.v1" {
                return Err(format!(
                    "unsupported finalized control namespace {}",
                    envelope.namespace
                ));
            }
            let command: ReplicatedControlCommand = serde_json::from_slice(&envelope.payload)
                .map_err(|error| format!("finalized control command decode failed: {error}"))?;
            effects.push(self.apply_command(
                command,
                finalized.block.height,
                &envelope.idempotency_key,
            )?);
        }
        *last_height = finalized.block.height;
        Ok(effects)
    }

    fn apply_command(
        &self,
        command: ReplicatedControlCommand,
        finalized_height: u64,
        idempotency_key: &str,
    ) -> Result<ProjectedControlEffect, String> {
        match command {
            ReplicatedControlCommand::RegisterZoneHost {
                registration,
                heartbeat,
            } => {
                let host_id = registration.host_id.clone();
                self.scheduler.register_host(registration, heartbeat)?;
                Ok(ProjectedControlEffect::ZoneHostRegistered(host_id))
            }
            ReplicatedControlCommand::ZoneHostHeartbeat { host_id, heartbeat } => {
                self.scheduler.heartbeat(&host_id, heartbeat)?;
                Ok(ProjectedControlEffect::ZoneHostHeartbeatAccepted(host_id))
            }
            ReplicatedControlCommand::PlaceZone { zone_id, now_ms } => self
                .scheduler
                .place_zone(zone_id, now_ms)
                .map(ProjectedControlEffect::ZonePlaced),
            ReplicatedControlCommand::BeginZoneHostDrain { host_id, now_ms } => self
                .scheduler
                .begin_drain(&host_id, now_ms)
                .map(ProjectedControlEffect::ZoneHostDrainStarted),
            ReplicatedControlCommand::FinishZoneHostDrain { host_id } => {
                self.scheduler.finish_drain(&host_id)?;
                Ok(ProjectedControlEffect::ZoneHostDrainFinished(host_id))
            }
            ReplicatedControlCommand::AdmitGuildNode { admission, now_ms } => {
                let node_id = admission.node_id.clone();
                self.guild_security.admit(admission, now_ms)?;
                Ok(ProjectedControlEffect::GuildNodeAdmitted(node_id))
            }
            ReplicatedControlCommand::RevokeGuildNode { node_id } => {
                let existed = self.guild_security.revoke(&node_id);
                Ok(ProjectedControlEffect::GuildNodeRevoked { node_id, existed })
            }
            ReplicatedControlCommand::SyncFinalizedGuildNode {
                registration,
                capacity_certificate,
                now_ms,
            } => {
                registration.validate()?;
                if idempotency_key != registration.finality.idempotency_key() {
                    return Err(
                        "finalized guild node command idempotency key does not match Sui event"
                            .to_string(),
                    );
                }
                if capacity_certificate.finalized_control_height > finalized_height {
                    return Err(
                        "capacity certificate depends on a future Commonware height".to_string()
                    );
                }
                let node_id = registration.node_id.clone();
                let checkpoint = registration.finality.checkpoint;
                self.guild_security.admit_finalized(
                    registration.clone(),
                    capacity_certificate.clone(),
                    now_ms,
                )?;
                if let Some(ledger) = self.reward_ledger.as_ref() {
                    ledger
                        .lock()
                        .map_err(|_| "reward ledger mutex poisoned".to_string())?
                        .register_node_eligibility(&registration, &capacity_certificate)?;
                }
                Ok(ProjectedControlEffect::FinalizedGuildNodeSynced {
                    node_id,
                    sui_checkpoint: checkpoint,
                })
            }
            ReplicatedControlCommand::RevokeFinalizedGuildNode { node_id, finality } => {
                finality.validate()?;
                if idempotency_key != finality.idempotency_key() {
                    return Err(
                        "guild node revocation idempotency key does not match Sui event"
                            .to_string(),
                    );
                }
                let checkpoint = finality.checkpoint;
                let existed = self.guild_security.revoke(&node_id);
                if let Some(ledger) = self.reward_ledger.as_ref() {
                    ledger
                        .lock()
                        .map_err(|_| "reward ledger mutex poisoned".to_string())?
                        .revoke_node_eligibility(&node_id);
                }
                Ok(ProjectedControlEffect::FinalizedGuildNodeRevoked {
                    node_id,
                    sui_checkpoint: checkpoint,
                    existed,
                })
            }
            ReplicatedControlCommand::RegisterGameRewardPolicy { policy } => {
                let game_id = policy.game_id.clone();
                let epoch = policy.epoch;
                self.reward_ledger()?
                    .lock()
                    .map_err(|_| "reward ledger mutex poisoned".to_string())?
                    .register_policy(policy)?;
                Ok(ProjectedControlEffect::RewardPolicyRegistered { game_id, epoch })
            }
            ReplicatedControlCommand::FinalizeGameRewardEpoch { game_id, epoch } => {
                let batch = self
                    .reward_ledger()?
                    .lock()
                    .map_err(|_| "reward ledger mutex poisoned".to_string())?
                    .finalize_epoch(&game_id, epoch, finalized_height)?;
                Ok(ProjectedControlEffect::RewardEpochFinalized(batch))
            }
        }
    }

    fn reward_ledger(&self) -> Result<&Arc<Mutex<MultiGameRewardLedger>>, String> {
        self.reward_ledger
            .as_ref()
            .ok_or_else(|| "finalized control projector has no reward ledger".to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsensusEquivocationEvidence {
    pub validator_id: String,
    pub epoch: u64,
    pub height: u64,
    pub view: u64,
    pub first_digest: String,
    pub second_digest: String,
}

#[derive(Debug)]
struct PendingControlBlock {
    block: ControlBlock,
    votes: BTreeSet<String>,
}

#[derive(Debug)]
struct ConsensusLogState {
    epoch: u64,
    next_view: u64,
    finalized: Vec<FinalizedControlBlock>,
    pending: BTreeMap<String, PendingControlBlock>,
    validator_votes: BTreeMap<(String, u64, u64, u64), String>,
    applied_idempotency_keys: BTreeSet<String>,
    equivocations: Vec<ConsensusEquivocationEvidence>,
}

impl Default for ConsensusLogState {
    fn default() -> Self {
        Self {
            epoch: 0,
            next_view: 0,
            finalized: Vec::new(),
            pending: BTreeMap::new(),
            validator_votes: BTreeMap::new(),
            applied_idempotency_keys: BTreeSet::new(),
            equivocations: Vec::new(),
        }
    }
}

/// Application-side adapter for Commonware Simplex finality. The production
/// network feeds validator votes/finalization into this state machine; tests use
/// the same collector to prove quorum, chaining, replay, and equivocation rules.
#[derive(Debug)]
pub struct CommonwareControlLog {
    committee: BTreeSet<String>,
    quorum: usize,
    state: Mutex<ConsensusLogState>,
}

impl CommonwareControlLog {
    pub fn new(committee: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let committee = committee.into_iter().collect::<BTreeSet<_>>();
        if committee.is_empty() {
            return Err("Commonware committee must not be empty".to_string());
        }
        for validator in &committee {
            validate_component("Commonware validator id", validator)?;
        }
        // n - floor((n - 1) / 3) == 2f + 1 for n == 3f + 1, and is the
        // conservative Byzantine quorum for partially filled committees.
        let faults = committee.len().saturating_sub(1) / 3;
        let quorum = committee.len().saturating_sub(faults);
        Ok(Self {
            committee,
            quorum,
            state: Mutex::new(ConsensusLogState::default()),
        })
    }

    pub fn quorum(&self) -> usize {
        self.quorum
    }

    pub fn propose(
        &self,
        proposer: &str,
        commands: Vec<ControlCommandEnvelope>,
    ) -> Result<ControlBlock, String> {
        if !self.committee.contains(proposer) {
            return Err(format!("unknown Commonware proposer {proposer}"));
        }
        let mut state = self.lock_state()?;
        for command in &commands {
            if state
                .applied_idempotency_keys
                .contains(&command.idempotency_key)
            {
                return Err(format!(
                    "control command {} is already finalized",
                    command.idempotency_key
                ));
            }
        }
        let height = state.finalized.len() as u64 + 1;
        let parent_digest = state
            .finalized
            .last()
            .map(|finalized| finalized.block.digest.clone())
            .unwrap_or_else(genesis_digest);
        let block = ControlBlock::new(
            state.epoch,
            height,
            state.next_view,
            parent_digest,
            proposer.to_string(),
            commands,
        )?;
        state.next_view = state.next_view.saturating_add(1);
        state.pending.insert(
            block.digest.clone(),
            PendingControlBlock {
                block: block.clone(),
                votes: BTreeSet::new(),
            },
        );
        Ok(block)
    }

    pub fn vote(
        &self,
        validator_id: &str,
        block_digest: &str,
    ) -> Result<Option<FinalizedControlBlock>, String> {
        if !self.committee.contains(validator_id) {
            return Err(format!("unknown Commonware validator {validator_id}"));
        }
        let mut state = self.lock_state()?;
        let block = state
            .pending
            .get(block_digest)
            .map(|pending| pending.block.clone())
            .ok_or_else(|| format!("unknown pending control block {block_digest}"))?;
        let vote_key = (
            validator_id.to_string(),
            block.epoch,
            block.height,
            block.view,
        );
        if let Some(first_digest) = state.validator_votes.get(&vote_key) {
            if first_digest != block_digest {
                let evidence = ConsensusEquivocationEvidence {
                    validator_id: validator_id.to_string(),
                    epoch: block.epoch,
                    height: block.height,
                    view: block.view,
                    first_digest: first_digest.clone(),
                    second_digest: block_digest.to_string(),
                };
                state.equivocations.push(evidence);
                return Err(format!(
                    "Commonware validator {validator_id} equivocated at epoch {} height {} view {}",
                    block.epoch, block.height, block.view
                ));
            }
        } else {
            state
                .validator_votes
                .insert(vote_key, block_digest.to_string());
        }
        let pending = state
            .pending
            .get_mut(block_digest)
            .expect("pending block checked above");
        pending.votes.insert(validator_id.to_string());
        if pending.votes.len() < self.quorum {
            return Ok(None);
        }
        self.finalize_locked(&mut state, block_digest).map(Some)
    }

    pub fn import_finalized(&self, finalized: FinalizedControlBlock) -> Result<(), String> {
        let mut state = self.lock_state()?;
        finalized.block.verify_digest()?;
        if finalized.signers.len() < self.quorum {
            return Err(format!(
                "Commonware finalization needs {} signers, got {}",
                self.quorum,
                finalized.signers.len()
            ));
        }
        if !finalized
            .signers
            .iter()
            .all(|signer| self.committee.contains(signer))
        {
            return Err("Commonware finalization contains an unknown signer".to_string());
        }
        validate_next_finalized(&state, &finalized.block)?;
        apply_finalized(&mut state, finalized)?;
        Ok(())
    }

    pub fn finalized(&self) -> Vec<FinalizedControlBlock> {
        self.state
            .lock()
            .map(|state| state.finalized.clone())
            .unwrap_or_default()
    }

    pub fn finalized_since(&self, height_exclusive: u64) -> Vec<FinalizedControlBlock> {
        self.finalized()
            .into_iter()
            .filter(|finalized| finalized.block.height > height_exclusive)
            .collect()
    }

    pub fn equivocations(&self) -> Vec<ConsensusEquivocationEvidence> {
        self.state
            .lock()
            .map(|state| state.equivocations.clone())
            .unwrap_or_default()
    }

    pub fn rotate_epoch(&self, expected_finalized_height: u64) -> Result<u64, String> {
        let mut state = self.lock_state()?;
        if state.finalized.len() as u64 != expected_finalized_height {
            return Err(format!(
                "cannot rotate Commonware epoch at stale height {expected_finalized_height}; current {}",
                state.finalized.len()
            ));
        }
        if !state.pending.is_empty() {
            return Err("cannot rotate Commonware epoch with pending blocks".to_string());
        }
        state.epoch = state.epoch.saturating_add(1);
        state.next_view = 0;
        Ok(state.epoch)
    }

    fn finalize_locked(
        &self,
        state: &mut ConsensusLogState,
        block_digest: &str,
    ) -> Result<FinalizedControlBlock, String> {
        let pending = state
            .pending
            .remove(block_digest)
            .ok_or_else(|| format!("unknown pending control block {block_digest}"))?;
        validate_next_finalized(state, &pending.block)?;
        let finalized = FinalizedControlBlock {
            block: pending.block,
            signers: pending.votes,
        };
        apply_finalized(state, finalized.clone())?;
        // Competing proposals at the finalized height can never commit.
        state
            .pending
            .retain(|_, candidate| candidate.block.height != finalized.block.height);
        Ok(finalized)
    }

    fn lock_state(&self) -> Result<std::sync::MutexGuard<'_, ConsensusLogState>, String> {
        self.state
            .lock()
            .map_err(|_| "Commonware control log mutex poisoned".to_string())
    }
}

fn validate_next_finalized(state: &ConsensusLogState, block: &ControlBlock) -> Result<(), String> {
    let expected_height = state.finalized.len() as u64 + 1;
    if block.height != expected_height {
        return Err(format!(
            "Commonware finalized height gap: expected {expected_height}, got {}",
            block.height
        ));
    }
    let expected_parent = state
        .finalized
        .last()
        .map(|finalized| finalized.block.digest.clone())
        .unwrap_or_else(genesis_digest);
    if block.parent_digest != expected_parent {
        return Err(format!(
            "Commonware finalized parent mismatch at height {}",
            block.height
        ));
    }
    for command in &block.commands {
        if state
            .applied_idempotency_keys
            .contains(&command.idempotency_key)
        {
            return Err(format!(
                "control command {} is already finalized",
                command.idempotency_key
            ));
        }
    }
    Ok(())
}

fn apply_finalized(
    state: &mut ConsensusLogState,
    finalized: FinalizedControlBlock,
) -> Result<(), String> {
    for command in &finalized.block.commands {
        if !state
            .applied_idempotency_keys
            .insert(command.idempotency_key.clone())
        {
            return Err(format!(
                "control command {} is already finalized",
                command.idempotency_key
            ));
        }
    }
    state.epoch = state.epoch.max(finalized.block.epoch);
    state.finalized.push(finalized);
    Ok(())
}

fn control_block_digest(
    epoch: u64,
    height: u64,
    view: u64,
    parent_digest: &str,
    proposer: &str,
    commands: &[ControlCommandEnvelope],
) -> Result<String, String> {
    let mut hash = Sha256::new();
    hash.update(CONTROL_BLOCK_DOMAIN);
    hash.update(epoch.to_be_bytes());
    hash.update(height.to_be_bytes());
    hash.update(view.to_be_bytes());
    hash_field(&mut hash, parent_digest.as_bytes());
    hash_field(&mut hash, proposer.as_bytes());
    let command_bytes = serde_json::to_vec(commands)
        .map_err(|error| format!("control block encode failed: {error}"))?;
    hash_field(&mut hash, &command_bytes);
    Ok(hex_digest(hash.finalize().as_slice()))
}

fn hash_field(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_be_bytes());
    hash.update(bytes);
}

fn genesis_digest() -> String {
    let mut hash = Sha256::new();
    hash.update(b"obelisk.mir2.control-genesis.v1");
    hex_digest(hash.finalize().as_slice())
}

fn hex_digest(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn validate_component(label: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{label} must not be empty"));
    }
    if value.len() > 255 || value.chars().any(char::is_control) {
        return Err(format!("invalid {label}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CapacityChallenge, CapacityChallengeResponse, CapacityWorkload, GuildNodeCapability,
        GuildNodeStatus, NodeSigningIdentity,
    };
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;

    fn committee() -> Vec<String> {
        ["validator-a", "validator-b", "validator-c", "validator-d"]
            .into_iter()
            .map(str::to_string)
            .collect()
    }

    fn command(key: &str) -> ControlCommandEnvelope {
        ControlCommandEnvelope::json(
            "zone-placement",
            key,
            &serde_json::json!({"zoneId": "map:0", "generation": 1}),
        )
        .expect("control command")
    }

    fn permissionless_registration(
        node: &NodeSigningIdentity,
        issuer: &NodeSigningIdentity,
    ) -> (FinalizedGuildNodeRegistration, NodeCapacityCertificate) {
        let registration = FinalizedGuildNodeRegistration {
            node_id: node.node_id().to_string(),
            operator_sui_address: format!("0x{}", "11".repeat(32)),
            public_key: node.public_key().to_string(),
            endpoint: "node-a:7020".to_string(),
            failure_domain: "test-az-a".to_string(),
            stake_mist: 2_000_000,
            max_sessions: 128,
            max_zones: 8,
            key_generation: 1,
            status: GuildNodeStatus::Active,
            finality: SuiFinalityProof {
                network: "testnet".to_string(),
                package_id: format!("0x{}", "22".repeat(32)),
                transaction_digest: "register-node-a".to_string(),
                event_sequence: 0,
                checkpoint: 42,
            },
        };
        let challenge = CapacityChallenge {
            challenge_id: "capacity-node-a".to_string(),
            node_id: registration.node_id.clone(),
            nonce: URL_SAFE_NO_PAD.encode([3_u8; 32]),
            issued_at_ms: 1_000,
            expires_at_ms: 5_000,
            workload: CapacityWorkload {
                concurrent_sessions: 64,
                max_sessions_per_zone: 16,
                zone_count: 4,
                command_count: 100,
                maximum_p95_latency_ms: 50,
                minimum_success_bps: 9_900,
            },
        };
        let response =
            CapacityChallengeResponse::sign(challenge, node, 1, 100, 0, 20, "ab".repeat(32), 2_000)
                .unwrap();
        let certificate =
            NodeCapacityCertificate::issue(&response, &registration, issuer, 2_500, 10_000, 1)
                .unwrap();
        (registration, certificate)
    }

    #[test]
    fn finalizes_only_after_byzantine_quorum_and_never_emits_empty_blocks() {
        let log = CommonwareControlLog::new(committee()).expect("committee");
        assert_eq!(log.quorum(), 3);
        assert!(log.propose("validator-a", Vec::new()).is_err());
        let block = log
            .propose("validator-a", vec![command("place-map-0-v1")])
            .expect("event block");

        assert!(log
            .vote("validator-a", &block.digest)
            .expect("vote a")
            .is_none());
        assert!(log
            .vote("validator-b", &block.digest)
            .expect("vote b")
            .is_none());
        let finalized = log
            .vote("validator-c", &block.digest)
            .expect("vote c")
            .expect("third vote finalizes");

        assert_eq!(finalized.block.height, 1);
        assert_eq!(finalized.signers.len(), 3);
        assert_eq!(log.finalized_since(0), vec![finalized]);
    }

    #[test]
    fn imported_finality_replays_identically_and_rejects_forks_or_duplicates() {
        let source = CommonwareControlLog::new(committee()).expect("source committee");
        let first = source
            .propose("validator-a", vec![command("first")])
            .expect("first proposal");
        for validator in ["validator-a", "validator-b"] {
            assert!(source
                .vote(validator, &first.digest)
                .expect("partial vote")
                .is_none());
        }
        let first = source
            .vote("validator-c", &first.digest)
            .expect("final vote")
            .expect("first finalized");

        let restored = CommonwareControlLog::new(committee()).expect("restored committee");
        restored
            .import_finalized(first.clone())
            .expect("valid finalization should import");
        assert_eq!(restored.finalized(), vec![first.clone()]);
        assert!(restored.import_finalized(first).is_err());

        let mut fork = source
            .propose("validator-b", vec![command("second")])
            .expect("second proposal");
        fork.parent_digest = "00".repeat(32);
        fork.digest = control_block_digest(
            fork.epoch,
            fork.height,
            fork.view,
            &fork.parent_digest,
            &fork.proposer,
            &fork.commands,
        )
        .expect("fork digest");
        assert!(restored
            .import_finalized(FinalizedControlBlock {
                block: fork,
                signers: BTreeSet::from([
                    "validator-a".to_string(),
                    "validator-b".to_string(),
                    "validator-c".to_string(),
                ]),
            })
            .is_err());
    }

    #[test]
    fn finalized_commands_project_into_scheduler_and_guild_admission_state() {
        let log = CommonwareControlLog::new(committee()).expect("committee");
        let scheduler = Arc::new(ZoneHostControlPlane::new(100, 1_000, 0));
        let guild_security = Arc::new(GuildNodeSecurityRegistry::new(1, 1_000));
        let projector = FinalizedControlProjector::new(scheduler.clone(), guild_security.clone());
        let registration = ZoneHostRegistration {
            host_id: "host-a".to_string(),
            endpoint: "127.0.0.1:7300".to_string(),
            failure_domain: "az-a".to_string(),
            max_sessions: 100,
            max_sessions_per_zone: 20,
            max_zones: 10,
            weight: 100,
        };
        let heartbeat = ZoneHostHeartbeat {
            session_count: 0,
            busiest_zone_session_count: 0,
            active_connections: 0,
            observed_at_ms: 10,
        };
        let admission = GuildNodeAdmission::zone_executor("guild-a", "guild", 1_000);
        let first = log
            .propose(
                "validator-a",
                vec![
                    ReplicatedControlCommand::RegisterZoneHost {
                        registration,
                        heartbeat,
                    }
                    .envelope("register-host-a")
                    .expect("registration envelope"),
                    ReplicatedControlCommand::AdmitGuildNode {
                        admission,
                        now_ms: 10,
                    }
                    .envelope("admit-guild-a")
                    .expect("admission envelope"),
                ],
            )
            .expect("first proposal");
        for validator in ["validator-a", "validator-b"] {
            assert!(log
                .vote(validator, &first.digest)
                .expect("partial vote")
                .is_none());
        }
        let first = log
            .vote("validator-c", &first.digest)
            .expect("final vote")
            .expect("first finalized");
        let effects = projector.apply(&first).expect("first projection");
        assert_eq!(effects.len(), 2);
        assert_eq!(projector.last_height(), 1);
        assert!(guild_security.is_eligible("guild-a", crate::GuildNodeCapability::ExecuteZone, 10));

        let second = log
            .propose(
                "validator-b",
                vec![ReplicatedControlCommand::PlaceZone {
                    zone_id: ZoneId::new("map:0"),
                    now_ms: 10,
                }
                .envelope("place-map-0")
                .expect("place envelope")],
            )
            .expect("second proposal");
        for validator in ["validator-a", "validator-b"] {
            assert!(log
                .vote(validator, &second.digest)
                .expect("partial vote")
                .is_none());
        }
        let second = log
            .vote("validator-c", &second.digest)
            .expect("final vote")
            .expect("second finalized");
        let effects = projector.apply(&second).expect("second projection");
        assert!(matches!(
            effects.as_slice(),
            [ProjectedControlEffect::ZonePlaced(_)]
        ));
        assert_eq!(
            scheduler
                .placement(&ZoneId::new("map:0"))
                .expect("projected placement")
                .primary
                .host_id,
            "host-a"
        );
    }

    #[test]
    fn reward_policy_and_epoch_close_take_effect_only_after_commonware_finality() {
        let log = CommonwareControlLog::new(committee()).unwrap();
        let reward_ledger = Arc::new(Mutex::new(MultiGameRewardLedger::default()));
        let projector = FinalizedControlProjector::new(
            Arc::new(ZoneHostControlPlane::new(100, 1_000, 0)),
            Arc::new(GuildNodeSecurityRegistry::default()),
        )
        .with_reward_ledger(reward_ledger.clone());
        let policy = GameRewardPolicy {
            game_id: "mir2".to_string(),
            epoch: 5,
            reward_budget: 1_000,
            reward_per_work_unit: 10,
            max_reward_per_node: 1_000,
            minimum_availability_bps: 9_000,
            minimum_quorum: 2,
            settlement_coin_type: "0x2::sui::SUI".to_string(),
        };
        let policy_block = log
            .propose(
                "validator-a",
                vec![
                    ReplicatedControlCommand::RegisterGameRewardPolicy { policy }
                        .envelope("mir2-reward-policy-5")
                        .unwrap(),
                ],
            )
            .unwrap();
        assert!(log
            .vote("validator-a", &policy_block.digest)
            .unwrap()
            .is_none());
        assert!(log
            .vote("validator-b", &policy_block.digest)
            .unwrap()
            .is_none());
        assert!(reward_ledger
            .lock()
            .unwrap()
            .ingest_verified(crate::VerifiedWorkReceipt {
                receipt_id: "receipt-before-finality".to_string(),
                game_id: "mir2".to_string(),
                epoch: 5,
                zone_id: "map:0".to_string(),
                control_height: 1,
                placement_generation: 1,
                work_units: 10,
                availability_bps: 10_000,
                quorum_node_ids: vec!["guild-a".to_string(), "guild-b".to_string()],
                execution_commitment: "ab".repeat(32),
                observed_at_ms: 10,
            })
            .is_err());
        let policy_finalized = log
            .vote("validator-c", &policy_block.digest)
            .unwrap()
            .unwrap();
        assert!(matches!(
            projector.apply(&policy_finalized).unwrap().as_slice(),
            [ProjectedControlEffect::RewardPolicyRegistered { .. }]
        ));

        reward_ledger
            .lock()
            .unwrap()
            .ingest_verified(crate::VerifiedWorkReceipt {
                receipt_id: "receipt-1".to_string(),
                game_id: "mir2".to_string(),
                epoch: 5,
                zone_id: "map:0".to_string(),
                control_height: 1,
                placement_generation: 1,
                work_units: 10,
                availability_bps: 10_000,
                quorum_node_ids: vec!["guild-a".to_string(), "guild-b".to_string()],
                execution_commitment: "ab".repeat(32),
                observed_at_ms: 10,
            })
            .unwrap();
        let close = log
            .propose(
                "validator-b",
                vec![ReplicatedControlCommand::FinalizeGameRewardEpoch {
                    game_id: "mir2".to_string(),
                    epoch: 5,
                }
                .envelope("mir2-reward-close-5")
                .unwrap()],
            )
            .unwrap();
        assert!(log.vote("validator-a", &close.digest).unwrap().is_none());
        assert!(log.vote("validator-b", &close.digest).unwrap().is_none());
        let close = log.vote("validator-c", &close.digest).unwrap().unwrap();
        let effects = projector.apply(&close).unwrap();
        assert!(matches!(
            effects.as_slice(),
            [ProjectedControlEffect::RewardEpochFinalized(batch)]
                if batch.game_id == "mir2" && batch.finalized_control_height == 2
        ));
    }

    #[test]
    fn sui_finality_and_capacity_become_membership_only_after_commonware_quorum() {
        let node = NodeSigningIdentity::from_seed([7; 32]);
        let issuer = NodeSigningIdentity::from_seed([9; 32]);
        let (registration, capacity_certificate) = permissionless_registration(&node, &issuer);
        let security = Arc::new(
            GuildNodeSecurityRegistry::with_trusted_capacity_issuers(
                1,
                1_000,
                [issuer.public_key().to_string()],
            )
            .unwrap(),
        );
        let rewards = Arc::new(Mutex::new(MultiGameRewardLedger::default()));
        let projector = FinalizedControlProjector::new(
            Arc::new(ZoneHostControlPlane::new(100, 1_000, 0)),
            security.clone(),
        )
        .with_reward_ledger(rewards.clone());
        let log = CommonwareControlLog::new(committee()).unwrap();
        let sync_key = registration.finality.idempotency_key();
        let block = log
            .propose(
                "validator-a",
                vec![ReplicatedControlCommand::SyncFinalizedGuildNode {
                    registration: registration.clone(),
                    capacity_certificate,
                    now_ms: 3_000,
                }
                .envelope(sync_key)
                .unwrap()],
            )
            .unwrap();

        assert!(log.vote("validator-a", &block.digest).unwrap().is_none());
        assert!(log.vote("validator-b", &block.digest).unwrap().is_none());
        assert!(!security.is_eligible(
            &registration.node_id,
            GuildNodeCapability::ExecuteZone,
            3_000,
        ));
        assert!(rewards
            .lock()
            .unwrap()
            .node_eligibility(&registration.node_id)
            .is_none());

        let finalized = log.vote("validator-c", &block.digest).unwrap().unwrap();
        assert!(matches!(
            projector.apply(&finalized).unwrap().as_slice(),
            [ProjectedControlEffect::FinalizedGuildNodeSynced {
                node_id,
                sui_checkpoint: 42,
            }] if node_id == &registration.node_id
        ));
        assert!(security.is_eligible(
            &registration.node_id,
            GuildNodeCapability::ExecuteZone,
            3_000,
        ));
        assert!(rewards
            .lock()
            .unwrap()
            .node_eligibility(&registration.node_id)
            .is_some());

        let revocation = SuiFinalityProof {
            network: "testnet".to_string(),
            package_id: registration.finality.package_id.clone(),
            transaction_digest: "revoke-node-a".to_string(),
            event_sequence: 0,
            checkpoint: 43,
        };
        let revoke = log
            .propose(
                "validator-b",
                vec![ReplicatedControlCommand::RevokeFinalizedGuildNode {
                    node_id: registration.node_id.clone(),
                    finality: revocation.clone(),
                }
                .envelope(revocation.idempotency_key())
                .unwrap()],
            )
            .unwrap();
        for validator in ["validator-a", "validator-b"] {
            assert!(log.vote(validator, &revoke.digest).unwrap().is_none());
        }
        let revoke = log.vote("validator-c", &revoke.digest).unwrap().unwrap();
        assert!(matches!(
            projector.apply(&revoke).unwrap().as_slice(),
            [ProjectedControlEffect::FinalizedGuildNodeRevoked {
                node_id,
                sui_checkpoint: 43,
                existed: true,
            }] if node_id == &registration.node_id
        ));
        assert!(!security.is_eligible(
            &registration.node_id,
            GuildNodeCapability::ExecuteZone,
            3_100,
        ));
        assert!(rewards
            .lock()
            .unwrap()
            .node_eligibility(&registration.node_id)
            .is_none());
    }

    #[cfg(feature = "commonware-2026-2")]
    #[test]
    fn pinned_commonware_release_types_round_trip_block_coordinates() {
        let log = CommonwareControlLog::new(committee()).expect("committee");
        let block = log
            .propose("validator-a", vec![command("coordinates")])
            .expect("proposal");
        let (epoch, height, view) = block.commonware_coordinates();

        assert_eq!(epoch.get(), block.epoch);
        assert_eq!(height.get(), block.height);
        assert_eq!(view.get(), block.view);
    }
}
