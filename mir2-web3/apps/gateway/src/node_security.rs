use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use mir2_protocol::{encode_server_packet, ServerPacket};
use mir2_simulation::{ActiveSessionIdentity, WorldCommandExecution, WorldSnapshot};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::rewards::VerifiedWorkReceipt;
use crate::routing::{
    SharedZoneLiveOutboundSender, SharedZoneOwnerRpcTransport, ZoneLiveOutboundRegistration,
    ZoneOwnerCommandRequest, ZoneOwnerLease, ZoneOwnerRpcTransport,
};
use crate::{FinalizedGuildNodeRegistration, GuildNodeStatus, NodeCapacityCertificate};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GuildNodeCapability {
    ExecuteZone,
    ReplicateCheckpoint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildNodeAdmission {
    pub node_id: String,
    pub operator_id: String,
    pub expires_at_ms: u64,
    pub capabilities: BTreeSet<GuildNodeCapability>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finalized_registration: Option<FinalizedGuildNodeRegistration>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capacity_certificate: Option<NodeCapacityCertificate>,
}

impl GuildNodeAdmission {
    pub fn zone_executor(
        node_id: impl Into<String>,
        operator_id: impl Into<String>,
        expires_at_ms: u64,
    ) -> Self {
        Self {
            node_id: node_id.into(),
            operator_id: operator_id.into(),
            expires_at_ms,
            capabilities: BTreeSet::from([GuildNodeCapability::ExecuteZone]),
            finalized_registration: None,
            capacity_certificate: None,
        }
    }

    pub fn from_finalized(
        registration: FinalizedGuildNodeRegistration,
        certificate: NodeCapacityCertificate,
    ) -> Result<Self, String> {
        registration.validate()?;
        if registration.status != GuildNodeStatus::Active {
            return Err("cannot admit a revoked guild node".to_string());
        }
        if certificate.node_id != registration.node_id
            || certificate.public_key != registration.public_key
            || certificate.key_generation != registration.key_generation
        {
            return Err(
                "capacity certificate does not match finalized guild node registration".to_string(),
            );
        }
        if certificate.max_sessions > registration.max_sessions
            || certificate.max_zones > registration.max_zones
        {
            return Err(
                "capacity certificate exceeds the node's Sui-registered capacity".to_string(),
            );
        }
        Ok(Self {
            node_id: registration.node_id.clone(),
            operator_id: registration.operator_sui_address.clone(),
            expires_at_ms: certificate.expires_at_ms,
            capabilities: BTreeSet::from([
                GuildNodeCapability::ExecuteZone,
                GuildNodeCapability::ReplicateCheckpoint,
            ]),
            finalized_registration: Some(registration),
            capacity_certificate: Some(certificate),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildNodeSecuritySnapshot {
    pub admission: GuildNodeAdmission,
    pub strikes: u32,
    pub agreements: u64,
    pub disagreements: u64,
    pub quarantine_until_ms: u64,
}

#[derive(Debug, Clone)]
struct GuildNodeSecurityRecord {
    admission: GuildNodeAdmission,
    strikes: u32,
    agreements: u64,
    disagreements: u64,
    quarantine_until_ms: u64,
}

#[derive(Debug)]
pub struct GuildNodeSecurityRegistry {
    nodes: Mutex<BTreeMap<String, GuildNodeSecurityRecord>>,
    strike_limit: u32,
    quarantine_ms: u64,
    trusted_capacity_issuers: BTreeSet<String>,
}

impl GuildNodeSecurityRegistry {
    pub fn new(strike_limit: u32, quarantine_ms: u64) -> Self {
        Self {
            nodes: Mutex::new(BTreeMap::new()),
            strike_limit: strike_limit.max(1),
            quarantine_ms: quarantine_ms.max(1),
            trusted_capacity_issuers: BTreeSet::new(),
        }
    }

    pub fn with_trusted_capacity_issuers(
        strike_limit: u32,
        quarantine_ms: u64,
        issuers: impl IntoIterator<Item = String>,
    ) -> Result<Self, String> {
        let trusted_capacity_issuers = issuers.into_iter().collect::<BTreeSet<_>>();
        if trusted_capacity_issuers.is_empty() {
            return Err("permissionless guild registry needs a capacity issuer".to_string());
        }
        for issuer in &trusted_capacity_issuers {
            crate::validate_ed25519_public_key(issuer)?;
        }
        Ok(Self {
            nodes: Mutex::new(BTreeMap::new()),
            strike_limit: strike_limit.max(1),
            quarantine_ms: quarantine_ms.max(1),
            trusted_capacity_issuers,
        })
    }

    pub fn admit(&self, admission: GuildNodeAdmission, now_ms: u64) -> Result<(), String> {
        validate_identity("guild node id", &admission.node_id)?;
        validate_identity("guild operator id", &admission.operator_id)?;
        self.validate_admission_foundation(&admission, now_ms)?;
        if admission.expires_at_ms <= now_ms {
            return Err(format!(
                "guild node admission {} is already expired",
                admission.node_id
            ));
        }
        if admission.capabilities.is_empty() {
            return Err(format!(
                "guild node admission {} has no capabilities",
                admission.node_id
            ));
        }
        self.nodes
            .lock()
            .map_err(|_| "guild node security registry mutex poisoned".to_string())?
            .insert(
                admission.node_id.clone(),
                GuildNodeSecurityRecord {
                    admission,
                    strikes: 0,
                    agreements: 0,
                    disagreements: 0,
                    quarantine_until_ms: 0,
                },
            );
        Ok(())
    }

    pub fn admit_finalized(
        &self,
        registration: FinalizedGuildNodeRegistration,
        certificate: NodeCapacityCertificate,
        now_ms: u64,
    ) -> Result<(), String> {
        self.admit(
            GuildNodeAdmission::from_finalized(registration, certificate)?,
            now_ms,
        )
    }

    pub fn revoke(&self, node_id: &str) -> bool {
        self.nodes
            .lock()
            .ok()
            .and_then(|mut nodes| nodes.remove(node_id))
            .is_some()
    }

    pub fn is_eligible(&self, node_id: &str, capability: GuildNodeCapability, now_ms: u64) -> bool {
        self.nodes
            .lock()
            .ok()
            .and_then(|nodes| nodes.get(node_id).cloned())
            .is_some_and(|record| {
                record.admission.expires_at_ms > now_ms
                    && record.quarantine_until_ms <= now_ms
                    && record.admission.capabilities.contains(&capability)
                    && record
                        .admission
                        .capacity_certificate
                        .as_ref()
                        .is_none_or(|certificate| certificate.expires_at_ms > now_ms)
            })
    }

    pub fn snapshots(&self) -> Vec<GuildNodeSecuritySnapshot> {
        self.nodes
            .lock()
            .map(|nodes| {
                nodes
                    .values()
                    .map(|record| GuildNodeSecuritySnapshot {
                        admission: record.admission.clone(),
                        strikes: record.strikes,
                        agreements: record.agreements,
                        disagreements: record.disagreements,
                        quarantine_until_ms: record.quarantine_until_ms,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn record_agreement(&self, node_id: &str) {
        if let Ok(mut nodes) = self.nodes.lock() {
            if let Some(record) = nodes.get_mut(node_id) {
                record.agreements = record.agreements.saturating_add(1);
                record.strikes = record.strikes.saturating_sub(1);
            }
        }
    }

    fn record_disagreement(&self, node_id: &str, now_ms: u64) {
        if let Ok(mut nodes) = self.nodes.lock() {
            if let Some(record) = nodes.get_mut(node_id) {
                record.disagreements = record.disagreements.saturating_add(1);
                record.strikes = record.strikes.saturating_add(1);
                if record.strikes >= self.strike_limit {
                    record.quarantine_until_ms = now_ms.saturating_add(self.quarantine_ms);
                    record.strikes = 0;
                }
            }
        }
    }

    fn validate_admission_foundation(
        &self,
        admission: &GuildNodeAdmission,
        now_ms: u64,
    ) -> Result<(), String> {
        match (
            admission.finalized_registration.as_ref(),
            admission.capacity_certificate.as_ref(),
        ) {
            (Some(registration), Some(certificate)) => {
                registration.validate()?;
                if registration.status != GuildNodeStatus::Active {
                    return Err("guild node is revoked in finalized Sui state".to_string());
                }
                if registration.node_id != admission.node_id
                    || registration.operator_sui_address != admission.operator_id
                    || certificate.node_id != admission.node_id
                    || certificate.public_key != registration.public_key
                    || certificate.key_generation != registration.key_generation
                {
                    return Err(
                        "guild admission identity does not match finalized registration"
                            .to_string(),
                    );
                }
                if !self
                    .trusted_capacity_issuers
                    .contains(&certificate.issuer_public_key)
                {
                    return Err("guild capacity certificate issuer is not trusted".to_string());
                }
                certificate.verify(&certificate.issuer_public_key, now_ms)?;
                if admission.expires_at_ms != certificate.expires_at_ms {
                    return Err(
                        "guild admission expiry must match capacity certificate".to_string()
                    );
                }
                Ok(())
            }
            (None, None) if self.trusted_capacity_issuers.is_empty() => Ok(()),
            (None, None) => Err(
                "permissionless guild admission requires Sui finality and capacity proof".into(),
            ),
            _ => Err(
                "guild admission requires both finalized registration and capacity certificate"
                    .to_string(),
            ),
        }
    }
}

impl Default for GuildNodeSecurityRegistry {
    fn default() -> Self {
        Self::new(3, 60_000)
    }
}

#[derive(Clone)]
pub struct VerifiedGuildNode {
    pub node_id: String,
    pub transport: SharedZoneOwnerRpcTransport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedWorkMeterContext {
    pub game_id: String,
    pub epoch: u64,
    pub finalized_control_height: u64,
    pub placement_generation: u64,
    pub availability_bps: u16,
}

impl fmt::Debug for VerifiedGuildNode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedGuildNode")
            .field("node_id", &self.node_id)
            .field("transport", &"ZoneOwnerRpcTransport")
            .finish()
    }
}

/// Executes every authoritative command on independent admitted nodes and only
/// releases an output when a digest quorum agrees on packets and post-state.
pub struct VerifiedGuildZoneTransport {
    nodes: Vec<VerifiedGuildNode>,
    threshold: usize,
    registry: Arc<GuildNodeSecurityRegistry>,
    work_meter: Option<VerifiedWorkMeterContext>,
    verified_work_receipts: Mutex<Vec<VerifiedWorkReceipt>>,
    receipt_sequence: Mutex<u64>,
}

impl VerifiedGuildZoneTransport {
    pub fn new(
        nodes: Vec<VerifiedGuildNode>,
        threshold: usize,
        registry: Arc<GuildNodeSecurityRegistry>,
    ) -> Result<Self, String> {
        if nodes.is_empty() {
            return Err("verified guild Zone transport needs at least one node".to_string());
        }
        if threshold == 0 || threshold > nodes.len() {
            return Err(format!(
                "verified guild Zone threshold {threshold} is invalid for {} nodes",
                nodes.len()
            ));
        }
        let mut ids = BTreeSet::new();
        for node in &nodes {
            validate_identity("guild node id", &node.node_id)?;
            if !ids.insert(node.node_id.clone()) {
                return Err(format!("duplicate guild node id {}", node.node_id));
            }
        }
        Ok(Self {
            nodes,
            threshold,
            registry,
            work_meter: None,
            verified_work_receipts: Mutex::new(Vec::new()),
            receipt_sequence: Mutex::new(0),
        })
    }

    pub fn with_work_meter(mut self, context: VerifiedWorkMeterContext) -> Result<Self, String> {
        validate_identity("game id", &context.game_id)?;
        if context.availability_bps > 10_000 {
            return Err("guild work-meter availability exceeds 10000 bps".to_string());
        }
        self.work_meter = Some(context);
        Ok(self)
    }

    pub fn drain_verified_work_receipts(&self) -> Vec<VerifiedWorkReceipt> {
        self.verified_work_receipts
            .lock()
            .map(|mut receipts| receipts.drain(..).collect())
            .unwrap_or_default()
    }

    fn eligible_nodes(&self, now_ms: u64) -> Vec<&VerifiedGuildNode> {
        self.nodes
            .iter()
            .filter(|node| {
                self.registry
                    .is_eligible(&node.node_id, GuildNodeCapability::ExecuteZone, now_ms)
            })
            .collect()
    }

    fn ensure_eligible(&self, now_ms: u64) -> Result<Vec<&VerifiedGuildNode>, String> {
        let nodes = self.eligible_nodes(now_ms);
        if nodes.len() < self.threshold {
            return Err(format!(
                "guild Zone quorum unavailable: need {}, only {} admitted nodes eligible",
                self.threshold,
                nodes.len()
            ));
        }
        Ok(nodes)
    }

    fn select_quorum<T>(
        &self,
        responses: Vec<(String, String, T)>,
        failed_nodes: Vec<String>,
        now_ms: u64,
        operation: &str,
    ) -> Result<T, String> {
        self.select_quorum_with_attestors(responses, failed_nodes, now_ms, operation)
            .map(|(value, _, _)| value)
    }

    fn select_quorum_with_attestors<T>(
        &self,
        responses: Vec<(String, String, T)>,
        failed_nodes: Vec<String>,
        now_ms: u64,
        operation: &str,
    ) -> Result<(T, Vec<String>, String), String> {
        let mut groups: BTreeMap<String, Vec<(String, T)>> = BTreeMap::new();
        for (node_id, digest, value) in responses {
            groups.entry(digest).or_default().push((node_id, value));
        }
        let winning_digest = groups
            .iter()
            .max_by(|(left_digest, left), (right_digest, right)| {
                left.len()
                    .cmp(&right.len())
                    .then_with(|| right_digest.cmp(left_digest))
            })
            .and_then(|(digest, values)| (values.len() >= self.threshold).then(|| digest.clone()));
        let Some(winning_digest) = winning_digest else {
            for values in groups.values() {
                for (node_id, _) in values {
                    self.registry.record_disagreement(node_id, now_ms);
                }
            }
            for node_id in failed_nodes {
                self.registry.record_disagreement(&node_id, now_ms);
            }
            return Err(format!(
                "guild Zone {operation} digest quorum failed: need {}, groups {:?}",
                self.threshold,
                groups.values().map(Vec::len).collect::<Vec<_>>()
            ));
        };
        let mut winner = None;
        let mut attestors = Vec::new();
        for (digest, values) in groups {
            for (node_id, value) in values {
                if digest == winning_digest {
                    self.registry.record_agreement(&node_id);
                    attestors.push(node_id);
                    if winner.is_none() {
                        winner = Some(value);
                    }
                } else {
                    self.registry.record_disagreement(&node_id, now_ms);
                }
            }
        }
        for node_id in failed_nodes {
            self.registry.record_disagreement(&node_id, now_ms);
        }
        attestors.sort();
        winner
            .map(|value| (value, attestors, winning_digest))
            .ok_or_else(|| format!("guild Zone {operation} quorum produced no value"))
    }

    fn record_verified_execution(
        &self,
        request: &ZoneOwnerCommandRequest,
        execution: &WorldCommandExecution,
        attestors: Vec<String>,
        commitment: String,
        observed_at_ms: u64,
    ) -> Result<(), String> {
        let Some(context) = self.work_meter.as_ref() else {
            return Ok(());
        };
        let mut sequence = self
            .receipt_sequence
            .lock()
            .map_err(|_| "guild reward receipt sequence mutex poisoned".to_string())?;
        *sequence = sequence.saturating_add(1);
        let receipt_id = verified_work_receipt_id(
            &context.game_id,
            context.epoch,
            request.owner_lease().zone_id().as_str(),
            request.owner_lease().fencing_token(),
            *sequence,
            &commitment,
        );
        let receipt = VerifiedWorkReceipt {
            receipt_id,
            game_id: context.game_id.clone(),
            epoch: context.epoch,
            zone_id: request.owner_lease().zone_id().as_str().to_string(),
            control_height: context.finalized_control_height,
            placement_generation: context.placement_generation,
            work_units: (execution.outcome.packet_count as u64).saturating_add(1),
            availability_bps: context.availability_bps,
            quorum_node_ids: attestors,
            execution_commitment: commitment,
            observed_at_ms,
        };
        receipt.validate()?;
        self.verified_work_receipts
            .lock()
            .map_err(|_| "guild reward receipt buffer mutex poisoned".to_string())?
            .push(receipt);
        Ok(())
    }
}

impl fmt::Debug for VerifiedGuildZoneTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedGuildZoneTransport")
            .field("nodes", &self.nodes)
            .field("threshold", &self.threshold)
            .field("work_meter", &self.work_meter)
            .finish_non_exhaustive()
    }
}

impl ZoneOwnerRpcTransport for VerifiedGuildZoneTransport {
    fn on_connect(&self) -> Result<Vec<ServerPacket>, String> {
        let now_ms = security_now_ms();
        let mut responses = Vec::new();
        let mut failed = Vec::new();
        for node in self.ensure_eligible(now_ms)? {
            match node.transport.on_connect() {
                Ok(packets) => {
                    responses.push((node.node_id.clone(), packet_commitment(&packets)?, packets))
                }
                Err(_) => failed.push(node.node_id.clone()),
            }
        }
        self.select_quorum(responses, failed, now_ms, "on_connect")
    }

    fn execute(&self, request: ZoneOwnerCommandRequest) -> Result<WorldCommandExecution, String> {
        let now_ms = security_now_ms();
        let mut responses = Vec::new();
        let mut failed = Vec::new();
        for node in self.ensure_eligible(now_ms)? {
            match node.transport.execute(request.clone()) {
                Ok(execution) => match node.transport.world_snapshot() {
                    Ok(snapshot) => responses.push((
                        node.node_id.clone(),
                        execution_commitment(&execution, &snapshot)?,
                        execution,
                    )),
                    Err(_) => failed.push(node.node_id.clone()),
                },
                Err(_) => failed.push(node.node_id.clone()),
            }
        }
        let (execution, attestors, commitment) =
            self.select_quorum_with_attestors(responses, failed, now_ms, "execute")?;
        self.record_verified_execution(&request, &execution, attestors, commitment, now_ms)?;
        Ok(execution)
    }

    fn world_snapshot(&self) -> Result<WorldSnapshot, String> {
        let now_ms = security_now_ms();
        let mut responses = Vec::new();
        let mut failed = Vec::new();
        for node in self.ensure_eligible(now_ms)? {
            match node.transport.world_snapshot() {
                Ok(snapshot) => responses.push((
                    node.node_id.clone(),
                    snapshot_commitment(&snapshot)?,
                    snapshot,
                )),
                Err(_) => failed.push(node.node_id.clone()),
            }
        }
        self.select_quorum(responses, failed, now_ms, "world_snapshot")
    }

    fn active_identity(&self) -> Result<Option<ActiveSessionIdentity>, String> {
        let now_ms = security_now_ms();
        let mut responses = Vec::new();
        let mut failed = Vec::new();
        for node in self.ensure_eligible(now_ms)? {
            match node.transport.active_identity() {
                Ok(identity) => {
                    responses.push((node.node_id.clone(), json_commitment(&identity)?, identity))
                }
                Err(_) => failed.push(node.node_id.clone()),
            }
        }
        self.select_quorum(responses, failed, now_ms, "active_identity")
    }

    fn save_active_character(&self) -> Result<(), String> {
        self.quorum_unit("save_active_character", |transport| {
            transport.save_active_character()
        })
    }

    fn refresh_active_external_mail(&self) -> Result<bool, String> {
        let now_ms = security_now_ms();
        let mut responses = Vec::new();
        let mut failed = Vec::new();
        for node in self.ensure_eligible(now_ms)? {
            match node.transport.refresh_active_external_mail() {
                Ok(value) => responses.push((node.node_id.clone(), value.to_string(), value)),
                Err(_) => failed.push(node.node_id.clone()),
            }
        }
        self.select_quorum(responses, failed, now_ms, "refresh_active_external_mail")
    }

    fn close_session(&self, owner_lease: &ZoneOwnerLease) -> Result<(), String> {
        self.quorum_unit("close_session", |transport| {
            transport.close_session(owner_lease)
        })
    }

    fn register_live_outbound(
        &self,
        sender: SharedZoneLiveOutboundSender,
    ) -> Result<Option<Box<dyn ZoneLiveOutboundRegistration>>, String> {
        let now_ms = security_now_ms();
        let primary = self
            .ensure_eligible(now_ms)?
            .into_iter()
            .next()
            .ok_or_else(|| "guild Zone live primary unavailable".to_string())?;
        primary.transport.register_live_outbound(sender)
    }
}

impl VerifiedGuildZoneTransport {
    fn quorum_unit(
        &self,
        operation: &str,
        call: impl Fn(&SharedZoneOwnerRpcTransport) -> Result<(), String>,
    ) -> Result<(), String> {
        let now_ms = security_now_ms();
        let nodes = self.ensure_eligible(now_ms)?;
        let mut successes = Vec::new();
        let mut failures = Vec::new();
        for node in nodes {
            match call(&node.transport) {
                Ok(()) => successes.push(node.node_id.clone()),
                Err(_) => failures.push(node.node_id.clone()),
            }
        }
        if successes.len() < self.threshold {
            let success_count = successes.len();
            for node_id in successes.into_iter().chain(failures) {
                self.registry.record_disagreement(&node_id, now_ms);
            }
            return Err(format!(
                "guild Zone {operation} quorum failed: need {}, got {}",
                self.threshold, success_count
            ));
        }
        for node_id in successes {
            self.registry.record_agreement(&node_id);
        }
        for node_id in failures {
            self.registry.record_disagreement(&node_id, now_ms);
        }
        Ok(())
    }
}

fn execution_commitment(
    execution: &WorldCommandExecution,
    snapshot: &WorldSnapshot,
) -> Result<String, String> {
    let mut hash = Sha256::new();
    hash.update(b"obelisk.mir2.guild-execution.v1\0");
    hash.update(packet_commitment(&execution.packets)?.as_bytes());
    hash.update(format!("{:?}", execution.outcome.command_kind).as_bytes());
    hash.update(execution.outcome.packet_count.to_be_bytes());
    hash.update(execution.outcome.snapshot_tick.to_be_bytes());
    hash.update(json_commitment(&execution.outcome.active_identity)?.as_bytes());
    hash.update(snapshot_commitment(snapshot)?.as_bytes());
    Ok(hex_digest(hash.finalize().as_slice()))
}

fn verified_work_receipt_id(
    game_id: &str,
    epoch: u64,
    zone_id: &str,
    fencing_token: u64,
    sequence: u64,
    commitment: &str,
) -> String {
    let mut hash = Sha256::new();
    hash.update(b"obelisk.shared-compute.verified-work.v1\0");
    hash.update((game_id.len() as u64).to_be_bytes());
    hash.update(game_id.as_bytes());
    hash.update(epoch.to_be_bytes());
    hash.update((zone_id.len() as u64).to_be_bytes());
    hash.update(zone_id.as_bytes());
    hash.update(fencing_token.to_be_bytes());
    hash.update(sequence.to_be_bytes());
    hash.update(commitment.as_bytes());
    hex_digest(hash.finalize().as_slice())
}

fn packet_commitment(packets: &[ServerPacket]) -> Result<String, String> {
    let mut hash = Sha256::new();
    hash.update(b"obelisk.mir2.guild-packets.v1\0");
    for packet in packets {
        let bytes = encode_server_packet(packet)
            .map_err(|error| format!("guild packet commitment encode failed: {error}"))?;
        hash.update((bytes.len() as u64).to_be_bytes());
        hash.update(bytes);
    }
    Ok(hex_digest(hash.finalize().as_slice()))
}

fn snapshot_commitment(snapshot: &WorldSnapshot) -> Result<String, String> {
    json_commitment(snapshot)
}

fn json_commitment(value: &impl serde::Serialize) -> Result<String, String> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| format!("guild verifier commitment encode failed: {error}"))?;
    let mut hash = Sha256::new();
    hash.update(b"obelisk.mir2.guild-json.v1\0");
    hash.update(bytes);
    Ok(hex_digest(hash.finalize().as_slice()))
}

fn hex_digest(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn security_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

fn validate_identity(label: &str, value: &str) -> Result<(), String> {
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
        CapacityChallenge, CapacityChallengeResponse, CapacityWorkload, GatewayConfig,
        HostedZoneOwnerCommandClient, InMemoryZoneOwnerLeaseAuthority, NodeSigningIdentity,
        SharedInProcessZoneRuntimeFactory, SharedZoneOwnerLeaseAuthority, SuiFinalityProof, ZoneId,
        ZoneOwnerCommandRequest, ZoneOwnerLeaseAuthority, ZoneRuntimeFactory,
    };
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    use mir2_protocol::{ClientPacket, ServerPacket};
    use mir2_simulation::WorldCommand;

    fn admitted_registry(node_ids: &[&str]) -> Arc<GuildNodeSecurityRegistry> {
        let registry = Arc::new(GuildNodeSecurityRegistry::new(1, 60_000));
        let now = security_now_ms();
        for node_id in node_ids {
            registry
                .admit(
                    GuildNodeAdmission::zone_executor(*node_id, "guild", now + 60_000),
                    now,
                )
                .expect("node admission");
        }
        registry
    }

    fn honest_node(node_id: &str, authority: SharedZoneOwnerLeaseAuthority) -> VerifiedGuildNode {
        let factory = SharedInProcessZoneRuntimeFactory::new();
        let runtime = factory.create_runtime(GatewayConfig::default(), &ZoneId::primary());
        VerifiedGuildNode {
            node_id: node_id.to_string(),
            transport: Arc::new(HostedZoneOwnerCommandClient::with_owner_lease_authority(
                runtime, authority,
            )),
        }
    }

    fn finalized_admission(
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
            challenge_id: "challenge-a".to_string(),
            node_id: registration.node_id.clone(),
            nonce: URL_SAFE_NO_PAD.encode([3_u8; 32]),
            issued_at_ms: 1_000,
            expires_at_ms: 5_000,
            workload: CapacityWorkload {
                concurrent_sessions: 64,
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

    #[derive(Debug)]
    struct DivergentTransport {
        inner: SharedZoneOwnerRpcTransport,
    }

    impl ZoneOwnerRpcTransport for DivergentTransport {
        fn execute(
            &self,
            request: ZoneOwnerCommandRequest,
        ) -> Result<WorldCommandExecution, String> {
            let mut execution = self.inner.execute(request)?;
            execution
                .packets
                .push(ServerPacket::KeepAlive { time: 999_999 });
            execution.outcome.packet_count = execution.packets.len();
            Ok(execution)
        }

        fn world_snapshot(&self) -> Result<WorldSnapshot, String> {
            self.inner.world_snapshot()
        }

        fn active_identity(&self) -> Result<Option<ActiveSessionIdentity>, String> {
            self.inner.active_identity()
        }

        fn save_active_character(&self) -> Result<(), String> {
            self.inner.save_active_character()
        }

        fn refresh_active_external_mail(&self) -> Result<bool, String> {
            self.inner.refresh_active_external_mail()
        }
    }

    #[test]
    fn two_of_three_quorum_releases_honest_execution_and_quarantines_divergence() {
        let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
        let shared: SharedZoneOwnerLeaseAuthority = authority.clone();
        let honest_a = honest_node("honest-a", shared.clone());
        let honest_b = honest_node("honest-b", shared.clone());
        let divergent_inner = honest_node("divergent-inner", shared);
        let divergent = VerifiedGuildNode {
            node_id: "divergent".to_string(),
            transport: Arc::new(DivergentTransport {
                inner: divergent_inner.transport,
            }),
        };
        let registry = admitted_registry(&["honest-a", "honest-b", "divergent"]);
        let transport = VerifiedGuildZoneTransport::new(
            vec![honest_a, honest_b, divergent],
            2,
            registry.clone(),
        )
        .expect("2-of-3 verifier");
        let lease = authority.owner_lease(&ZoneId::primary());

        let execution = transport
            .execute(ZoneOwnerCommandRequest::direct(
                lease,
                WorldCommand::ClientPacket(ClientPacket::KeepAlive { time: 42 }),
            ))
            .expect("honest quorum should execute");

        assert_eq!(
            execution.packets,
            vec![ServerPacket::KeepAlive { time: 42 }]
        );
        let snapshots = registry.snapshots();
        assert!(snapshots
            .iter()
            .find(|node| node.admission.node_id == "divergent")
            .is_some_and(|node| node.quarantine_until_ms > security_now_ms()));
        assert_eq!(
            snapshots
                .iter()
                .filter(|node| node.admission.node_id.starts_with("honest"))
                .map(|node| node.agreements)
                .sum::<u64>(),
            2
        );
    }

    #[test]
    fn admission_expiry_fails_closed_before_execution() {
        let registry = Arc::new(GuildNodeSecurityRegistry::new(1, 60_000));
        let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
        let node = honest_node("expired", authority.clone());
        let now = security_now_ms();
        registry
            .admit(
                GuildNodeAdmission::zone_executor("expired", "guild", now + 1),
                now,
            )
            .expect("short admission");
        let transport = VerifiedGuildZoneTransport::new(vec![node], 1, registry)
            .expect("single verifier construction");
        std::thread::sleep(std::time::Duration::from_millis(2));

        let error = transport
            .execute(ZoneOwnerCommandRequest::direct(
                authority.owner_lease(&ZoneId::primary()),
                WorldCommand::Tick,
            ))
            .expect_err("expired admission must fail closed");
        assert!(error.contains("quorum unavailable"));
    }

    #[test]
    fn permissionless_admission_requires_sui_finality_and_trusted_capacity_issuer() {
        let node = NodeSigningIdentity::from_seed([7; 32]);
        let issuer = NodeSigningIdentity::from_seed([9; 32]);
        let untrusted = NodeSigningIdentity::from_seed([10; 32]);
        let (registration, certificate) = finalized_admission(&node, &issuer);
        let registry = GuildNodeSecurityRegistry::with_trusted_capacity_issuers(
            1,
            60_000,
            [issuer.public_key().to_string()],
        )
        .unwrap();

        assert!(registry
            .admit(
                GuildNodeAdmission::zone_executor(node.node_id(), "guild", 10_000),
                3_000,
            )
            .is_err());

        let wrong_registry = GuildNodeSecurityRegistry::with_trusted_capacity_issuers(
            1,
            60_000,
            [untrusted.public_key().to_string()],
        )
        .unwrap();
        assert!(wrong_registry
            .admit_finalized(registration.clone(), certificate.clone(), 3_000)
            .is_err());

        registry
            .admit_finalized(registration.clone(), certificate.clone(), 3_000)
            .unwrap();
        assert!(registry.is_eligible(
            &registration.node_id,
            GuildNodeCapability::ExecuteZone,
            3_000,
        ));
        assert!(!registry.is_eligible(
            &registration.node_id,
            GuildNodeCapability::ExecuteZone,
            certificate.expires_at_ms,
        ));
    }

    #[test]
    fn verified_execution_emits_rewardable_receipt_for_agreeing_nodes_only() {
        let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
        let shared: SharedZoneOwnerLeaseAuthority = authority.clone();
        let registry = admitted_registry(&["guild-a", "guild-b"]);
        let transport = VerifiedGuildZoneTransport::new(
            vec![
                honest_node("guild-a", shared.clone()),
                honest_node("guild-b", shared),
            ],
            2,
            registry,
        )
        .unwrap()
        .with_work_meter(VerifiedWorkMeterContext {
            game_id: "mir2".to_string(),
            epoch: 9,
            finalized_control_height: 27,
            placement_generation: 4,
            availability_bps: 9_950,
        })
        .unwrap();
        transport
            .execute(ZoneOwnerCommandRequest::direct(
                authority.owner_lease(&ZoneId::primary()),
                WorldCommand::ClientPacket(ClientPacket::KeepAlive { time: 5 }),
            ))
            .unwrap();

        let receipts = transport.drain_verified_work_receipts();
        assert_eq!(receipts.len(), 1);
        let receipt = &receipts[0];
        assert_eq!(receipt.game_id, "mir2");
        assert_eq!(receipt.epoch, 9);
        assert_eq!(receipt.control_height, 27);
        assert_eq!(receipt.placement_generation, 4);
        assert_eq!(receipt.quorum_node_ids, ["guild-a", "guild-b"]);
        assert!(receipt.work_units > 0);
        receipt.validate().unwrap();
        assert!(transport.drain_verified_work_receipts().is_empty());
    }
}
