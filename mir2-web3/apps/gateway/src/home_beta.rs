use std::collections::{BTreeMap, BTreeSet};
use std::net::IpAddr;
use std::sync::Mutex;

use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::{
    node_id_from_public_key, verify_ed25519_signature, GameRewardPolicy, HomeAgentWorkMode,
    NodeSigningIdentity, VerifiedWorkReceipt,
};

pub const HOME_TELEMETRY_SCHEMA: &str = "obelisk.home-node-telemetry.v1";
pub const HOME_BETA_RUN_SCHEMA: &str = "obelisk.home-network-beta-run.v1";
pub const HOME_BETA_COHORT_SCHEMA: &str = "obelisk.home-network-beta-cohort.v1";
pub const HOME_SIGNATURE_ALGORITHM: &str = "ed25519-zip215";
pub const HOME_BETA_MINIMUM_DURATION_MS: u64 = 15 * 60 * 1_000;
pub const HOME_BETA_MAXIMUM_FAILOVER_RTO_MS: u64 = 4_999;
pub const HOME_RELAY_PSEUDONYM_MINIMUM_SECRET_BYTES: usize = 32;

const TELEMETRY_DOMAIN: &[u8] = b"obelisk.home-node-telemetry.v1\0";
const BETA_NODE_DOMAIN: &[u8] = b"obelisk.home-network-beta-node.v1\0";
const BETA_OPERATOR_DOMAIN: &[u8] = b"obelisk.home-network-beta-operator.v1\0";
const RELAY_IP_DOMAIN: &[u8] = b"obelisk.home-relay-ip-pseudonym.v1\0";

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeNodeTelemetryPayload {
    pub schema: String,
    pub node_id: String,
    pub public_key: String,
    pub key_generation: u64,
    pub agent_instance_id: String,
    pub sequence: u64,
    pub window_started_at_ms: u64,
    pub observed_at_ms: u64,
    pub coarse_region: String,
    pub provider_code: String,
    pub relay_rtt_ms: u32,
    pub packet_loss_bps: u16,
    pub measured_upstream_kbps: u32,
    pub active_sessions: u32,
    pub active_zones: u16,
    pub zone_ids: Vec<String>,
    pub checkpoint_lag_ms: u32,
    pub cpu_usage_bps: u16,
    pub memory_usage_bps: u16,
    pub work_mode: HomeAgentWorkMode,
    pub capacity_certificate_id: String,
    pub capacity_certificate_expires_at_ms: u64,
    pub capacity_max_sessions: u32,
    pub capacity_max_zones: u16,
    pub finalized_control_height: u64,
    pub placement_generation: u64,
    pub game_id: String,
    pub reward_epoch: u64,
    pub verified_work_units: u64,
    pub session_milliseconds: u64,
    pub agent_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignedHomeNodeTelemetry {
    pub payload: HomeNodeTelemetryPayload,
    pub signature_algorithm: String,
    pub signature: String,
}

impl SignedHomeNodeTelemetry {
    pub fn sign(
        payload: HomeNodeTelemetryPayload,
        identity: &NodeSigningIdentity,
    ) -> Result<Self, String> {
        validate_telemetry_payload(&payload)?;
        if payload.node_id != identity.node_id() || payload.public_key != identity.public_key() {
            return Err("telemetry identity does not match signing identity".to_string());
        }
        let signature = identity.sign(&domain_json(TELEMETRY_DOMAIN, &payload)?);
        Ok(Self {
            payload,
            signature_algorithm: HOME_SIGNATURE_ALGORITHM.to_string(),
            signature,
        })
    }

    pub fn verify(&self, now_ms: u64, maximum_age_ms: u64) -> Result<(), String> {
        self.verify_signature()?;
        if now_ms < self.payload.observed_at_ms
            || now_ms.saturating_sub(self.payload.observed_at_ms) > maximum_age_ms
        {
            return Err("Home telemetry report is stale or from the future".to_string());
        }
        Ok(())
    }

    pub fn verify_signature(&self) -> Result<(), String> {
        validate_telemetry_payload(&self.payload)?;
        if self.signature_algorithm != HOME_SIGNATURE_ALGORITHM {
            return Err("unsupported Home telemetry signature algorithm".to_string());
        }
        verify_ed25519_signature(
            &self.payload.public_key,
            &domain_json(TELEMETRY_DOMAIN, &self.payload)?,
            &self.signature,
        )
    }
}

#[derive(Debug, Default)]
pub struct HomeTelemetryReplayGuard {
    sequences: Mutex<BTreeMap<(String, u64, String), u64>>,
}

#[derive(Debug)]
pub struct HomeTelemetryStore {
    maximum_report_age_ms: u64,
    retention_ms: u64,
    replay_guard: HomeTelemetryReplayGuard,
    reports: Mutex<BTreeMap<String, SignedHomeNodeTelemetry>>,
}

impl HomeTelemetryStore {
    pub fn new(maximum_report_age_ms: u64, retention_ms: u64) -> Result<Self, String> {
        if maximum_report_age_ms == 0
            || retention_ms < maximum_report_age_ms
            || retention_ms > 90 * 24 * 60 * 60 * 1_000
        {
            return Err("Home telemetry retention policy is invalid".to_string());
        }
        Ok(Self {
            maximum_report_age_ms,
            retention_ms,
            replay_guard: HomeTelemetryReplayGuard::default(),
            reports: Mutex::new(BTreeMap::new()),
        })
    }

    pub fn ingest(&self, report: SignedHomeNodeTelemetry, now_ms: u64) -> Result<(), String> {
        self.replay_guard
            .accept(&report, now_ms, self.maximum_report_age_ms)?;
        self.reports
            .lock()
            .map_err(|_| "Home telemetry store mutex poisoned".to_string())?
            .insert(report.payload.node_id.clone(), report);
        Ok(())
    }

    pub fn operator_view(
        &self,
        node_id: &str,
        now_ms: u64,
    ) -> Option<HomeNodeOperatorTelemetryView> {
        self.reports
            .lock()
            .ok()?
            .get(node_id)
            .filter(|report| {
                now_ms.saturating_sub(report.payload.observed_at_ms) <= self.retention_ms
            })
            .map(SignedHomeNodeTelemetry::operator_view)
    }

    pub fn public_view(
        &self,
        now_ms: u64,
        expected_reports: u32,
    ) -> Result<HomeNodePublicTelemetryView, String> {
        let reports = self
            .reports
            .lock()
            .map_err(|_| "Home telemetry store mutex poisoned".to_string())?
            .values()
            .filter(|report| {
                now_ms.saturating_sub(report.payload.observed_at_ms) <= self.retention_ms
            })
            .cloned()
            .collect::<Vec<_>>();
        aggregate_public_telemetry(&reports, expected_reports)
    }

    pub fn delete_node(&self, node_id: &str) -> Result<bool, String> {
        self.replay_guard.clear_node(node_id);
        Ok(self
            .reports
            .lock()
            .map_err(|_| "Home telemetry store mutex poisoned".to_string())?
            .remove(node_id)
            .is_some())
    }

    pub fn prune(&self, now_ms: u64) -> Result<usize, String> {
        let mut reports = self
            .reports
            .lock()
            .map_err(|_| "Home telemetry store mutex poisoned".to_string())?;
        let before = reports.len();
        reports.retain(|_, report| {
            now_ms.saturating_sub(report.payload.observed_at_ms) <= self.retention_ms
        });
        Ok(before.saturating_sub(reports.len()))
    }
}

impl HomeTelemetryReplayGuard {
    pub fn accept(
        &self,
        report: &SignedHomeNodeTelemetry,
        now_ms: u64,
        maximum_age_ms: u64,
    ) -> Result<(), String> {
        report.verify(now_ms, maximum_age_ms)?;
        let key = (
            report.payload.node_id.clone(),
            report.payload.key_generation,
            report.payload.agent_instance_id.clone(),
        );
        let mut sequences = self
            .sequences
            .lock()
            .map_err(|_| "Home telemetry replay guard mutex poisoned".to_string())?;
        if sequences
            .get(&key)
            .is_some_and(|sequence| report.payload.sequence <= *sequence)
        {
            return Err("replayed or reordered Home telemetry report".to_string());
        }
        sequences.insert(key, report.payload.sequence);
        Ok(())
    }

    pub fn clear_node(&self, node_id: &str) {
        if let Ok(mut sequences) = self.sequences.lock() {
            sequences.retain(|(registered, _, _), _| registered != node_id);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeNodeOwnerTelemetryView {
    pub node_id: String,
    pub observed_at_ms: u64,
    pub work_mode: HomeAgentWorkMode,
    pub active_sessions: u32,
    pub active_zones: u16,
    pub zone_ids: Vec<String>,
    pub cpu_usage_bps: u16,
    pub memory_usage_bps: u16,
    pub relay_rtt_ms: u32,
    pub packet_loss_bps: u16,
    pub measured_upstream_kbps: u32,
    pub checkpoint_lag_ms: u32,
    pub capacity_certificate_id: String,
    pub capacity_certificate_expires_at_ms: u64,
    pub verified_work_units: u64,
    pub session_milliseconds: u64,
    pub agent_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeNodeOperatorTelemetryView {
    pub node_id: String,
    pub key_generation: u64,
    pub agent_instance_id: String,
    pub sequence: u64,
    pub observed_at_ms: u64,
    pub coarse_region: String,
    pub provider_code: String,
    pub work_mode: HomeAgentWorkMode,
    pub active_sessions: u32,
    pub active_zones: u16,
    pub zone_ids: Vec<String>,
    pub relay_rtt_ms: u32,
    pub packet_loss_bps: u16,
    pub measured_upstream_kbps: u32,
    pub checkpoint_lag_ms: u32,
    pub capacity_certificate_id: String,
    pub capacity_certificate_expires_at_ms: u64,
    pub finalized_control_height: u64,
    pub placement_generation: u64,
    pub verified_work_units: u64,
    pub session_milliseconds: u64,
    pub agent_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeNodePublicTelemetryView {
    pub coarse_region: String,
    pub online_nodes: u32,
    pub serving_nodes: u32,
    pub active_sessions: u64,
    pub active_zones: u64,
    pub aggregate_capacity_sessions: u64,
    pub aggregate_capacity_zones: u64,
    pub availability_bps: u16,
    pub p95_relay_rtt_ms: u32,
    pub ip_hidden_by_regional_relay: bool,
}

impl SignedHomeNodeTelemetry {
    pub fn owner_view(&self) -> HomeNodeOwnerTelemetryView {
        let payload = &self.payload;
        HomeNodeOwnerTelemetryView {
            node_id: payload.node_id.clone(),
            observed_at_ms: payload.observed_at_ms,
            work_mode: payload.work_mode,
            active_sessions: payload.active_sessions,
            active_zones: payload.active_zones,
            zone_ids: payload.zone_ids.clone(),
            cpu_usage_bps: payload.cpu_usage_bps,
            memory_usage_bps: payload.memory_usage_bps,
            relay_rtt_ms: payload.relay_rtt_ms,
            packet_loss_bps: payload.packet_loss_bps,
            measured_upstream_kbps: payload.measured_upstream_kbps,
            checkpoint_lag_ms: payload.checkpoint_lag_ms,
            capacity_certificate_id: payload.capacity_certificate_id.clone(),
            capacity_certificate_expires_at_ms: payload.capacity_certificate_expires_at_ms,
            verified_work_units: payload.verified_work_units,
            session_milliseconds: payload.session_milliseconds,
            agent_version: payload.agent_version.clone(),
        }
    }

    pub fn operator_view(&self) -> HomeNodeOperatorTelemetryView {
        let payload = &self.payload;
        HomeNodeOperatorTelemetryView {
            node_id: payload.node_id.clone(),
            key_generation: payload.key_generation,
            agent_instance_id: payload.agent_instance_id.clone(),
            sequence: payload.sequence,
            observed_at_ms: payload.observed_at_ms,
            coarse_region: payload.coarse_region.clone(),
            provider_code: payload.provider_code.clone(),
            work_mode: payload.work_mode,
            active_sessions: payload.active_sessions,
            active_zones: payload.active_zones,
            zone_ids: payload.zone_ids.clone(),
            relay_rtt_ms: payload.relay_rtt_ms,
            packet_loss_bps: payload.packet_loss_bps,
            measured_upstream_kbps: payload.measured_upstream_kbps,
            checkpoint_lag_ms: payload.checkpoint_lag_ms,
            capacity_certificate_id: payload.capacity_certificate_id.clone(),
            capacity_certificate_expires_at_ms: payload.capacity_certificate_expires_at_ms,
            finalized_control_height: payload.finalized_control_height,
            placement_generation: payload.placement_generation,
            verified_work_units: payload.verified_work_units,
            session_milliseconds: payload.session_milliseconds,
            agent_version: payload.agent_version.clone(),
        }
    }
}

pub fn aggregate_public_telemetry(
    reports: &[SignedHomeNodeTelemetry],
    expected_reports: u32,
) -> Result<HomeNodePublicTelemetryView, String> {
    if reports.is_empty() || expected_reports == 0 {
        return Err("public telemetry aggregation requires reports and an expected cohort".into());
    }
    let mut regions = BTreeSet::new();
    let mut nodes = BTreeSet::new();
    let mut latencies = Vec::with_capacity(reports.len());
    let mut sessions = 0_u64;
    let mut zones = 0_u64;
    let mut capacity_sessions = 0_u64;
    let mut capacity_zones = 0_u64;
    let mut serving = 0_u32;
    for report in reports {
        report.verify_signature()?;
        if !nodes.insert(report.payload.node_id.clone()) {
            return Err("public telemetry aggregation contains duplicate nodes".into());
        }
        regions.insert(report.payload.coarse_region.clone());
        latencies.push(report.payload.relay_rtt_ms);
        sessions = sessions.saturating_add(u64::from(report.payload.active_sessions));
        zones = zones.saturating_add(u64::from(report.payload.active_zones));
        capacity_sessions =
            capacity_sessions.saturating_add(u64::from(report.payload.capacity_max_sessions));
        capacity_zones =
            capacity_zones.saturating_add(u64::from(report.payload.capacity_max_zones));
        if report.payload.work_mode == HomeAgentWorkMode::Serving {
            serving = serving.saturating_add(1);
        }
    }
    latencies.sort_unstable();
    let p95_index = ((latencies.len() * 95).saturating_add(99) / 100)
        .saturating_sub(1)
        .min(latencies.len() - 1);
    let availability_bps = ((reports.len() as u128)
        .saturating_mul(10_000)
        .checked_div(u128::from(expected_reports))
        .unwrap_or_default())
    .min(10_000) as u16;
    Ok(HomeNodePublicTelemetryView {
        coarse_region: if regions.len() == 1 {
            regions.into_iter().next().unwrap_or_default()
        } else {
            "multi-region".to_string()
        },
        online_nodes: reports.len().min(u32::MAX as usize) as u32,
        serving_nodes: serving,
        active_sessions: sessions,
        active_zones: zones,
        aggregate_capacity_sessions: capacity_sessions,
        aggregate_capacity_zones: capacity_zones,
        availability_bps,
        p95_relay_rtt_ms: latencies[p95_index],
        ip_hidden_by_regional_relay: true,
    })
}

pub fn relay_source_ip_pseudonym(
    rotating_secret: &[u8],
    rotation_id: &str,
    source_ip: IpAddr,
) -> Result<String, String> {
    if rotating_secret.len() < HOME_RELAY_PSEUDONYM_MINIMUM_SECRET_BYTES {
        return Err("Relay IP pseudonym secret must contain at least 32 bytes".to_string());
    }
    validate_label("Relay pseudonym rotation id", rotation_id, 128)?;
    let mut mac = HmacSha256::new_from_slice(rotating_secret)
        .map_err(|_| "Relay IP pseudonym secret is invalid".to_string())?;
    mac.update(RELAY_IP_DOMAIN);
    mac.update(&(rotation_id.len() as u64).to_be_bytes());
    mac.update(rotation_id.as_bytes());
    match source_ip {
        IpAddr::V4(address) => {
            mac.update(&[4]);
            mac.update(&address.octets());
        }
        IpAddr::V6(address) => {
            mac.update(&[6]);
            mac.update(&address.octets());
        }
    }
    Ok(hex_lower(&mac.finalize().into_bytes()))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeRewardReconciliation {
    pub node_id: String,
    pub game_id: String,
    pub epoch: u64,
    pub telemetry_work_units: u64,
    pub receipt_work_units: u64,
    pub accepted_work_score: u64,
    pub estimated_reward: u64,
    pub session_milliseconds: u64,
    pub accepted_receipt_ids: Vec<String>,
    pub discrepancies: Vec<String>,
    pub payable: bool,
}

pub fn reconcile_home_node_reward(
    report: &SignedHomeNodeTelemetry,
    receipts: &[VerifiedWorkReceipt],
    policy: &GameRewardPolicy,
    now_ms: u64,
    maximum_report_age_ms: u64,
) -> Result<HomeRewardReconciliation, String> {
    report.verify(now_ms, maximum_report_age_ms)?;
    policy.validate()?;
    let payload = &report.payload;
    if payload.game_id != policy.game_id || payload.reward_epoch != policy.epoch {
        return Err("telemetry reward scope does not match reward policy".to_string());
    }
    let mut seen = BTreeSet::new();
    let zones = payload.zone_ids.iter().cloned().collect::<BTreeSet<_>>();
    let mut accepted_receipt_ids = Vec::new();
    let mut receipt_work_units = 0_u64;
    let mut accepted_work_score = 0_u64;
    let mut discrepancies = Vec::new();
    for receipt in receipts {
        receipt.validate()?;
        if receipt.game_id != policy.game_id || receipt.epoch != policy.epoch {
            continue;
        }
        if !seen.insert(receipt.receipt_id.clone()) {
            discrepancies.push(format!("duplicate_receipt:{}", receipt.receipt_id));
            continue;
        }
        if !receipt.quorum_node_ids.contains(&payload.node_id) {
            continue;
        }
        let mismatch = if !zones.contains(&receipt.zone_id) {
            Some("zone")
        } else if receipt.placement_generation != payload.placement_generation {
            Some("placement_generation")
        } else if receipt.control_height > payload.finalized_control_height {
            Some("unfinalized_control_height")
        } else if receipt.observed_at_ms < payload.window_started_at_ms
            || receipt.observed_at_ms > payload.observed_at_ms
        {
            Some("outside_telemetry_window")
        } else if receipt.availability_bps < policy.minimum_availability_bps {
            Some("availability")
        } else if receipt.quorum_node_ids.len() < usize::from(policy.minimum_quorum) {
            Some("quorum")
        } else {
            None
        };
        if let Some(reason) = mismatch {
            discrepancies.push(format!("receipt_{}:{}", reason, receipt.receipt_id));
            continue;
        }
        receipt_work_units = receipt_work_units.saturating_add(receipt.work_units);
        accepted_work_score = accepted_work_score.saturating_add(
            receipt
                .work_units
                .saturating_mul(u64::from(receipt.availability_bps))
                / 10_000,
        );
        accepted_receipt_ids.push(receipt.receipt_id.clone());
    }
    if payload.verified_work_units != receipt_work_units {
        discrepancies.push(format!(
            "work_unit_mismatch:telemetry={}:receipts={receipt_work_units}",
            payload.verified_work_units
        ));
    }
    let accepted_work_score = if payload.verified_work_units == receipt_work_units {
        accepted_work_score
    } else {
        0
    };
    let estimated_reward = accepted_work_score
        .saturating_mul(policy.reward_per_work_unit)
        .min(policy.max_reward_per_node)
        .min(policy.reward_budget);
    let payable = accepted_work_score > 0 && discrepancies.is_empty();
    Ok(HomeRewardReconciliation {
        node_id: payload.node_id.clone(),
        game_id: payload.game_id.clone(),
        epoch: payload.reward_epoch,
        telemetry_work_units: payload.verified_work_units,
        receipt_work_units,
        accepted_work_score,
        estimated_reward,
        session_milliseconds: payload.session_milliseconds,
        accepted_receipt_ids,
        discrepancies,
        payable,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HomeBetaEnvironment {
    PhysicalHomeNetwork,
    LabNetwork,
    SimulatedNetwork,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HomeBetaFaultKind {
    CgnatBaseline,
    DynamicIpChange,
    RouterRestart,
    HostSleepWake,
    PacketLoss,
    BandwidthCongestion,
    ActiveFailureStandbyTakeover,
}

impl HomeBetaFaultKind {
    fn required() -> BTreeSet<Self> {
        BTreeSet::from([
            Self::CgnatBaseline,
            Self::DynamicIpChange,
            Self::RouterRestart,
            Self::HostSleepWake,
            Self::PacketLoss,
            Self::BandwidthCongestion,
            Self::ActiveFailureStandbyTakeover,
        ])
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeBetaFaultObservation {
    pub kind: HomeBetaFaultKind,
    pub injected_at_ms: u64,
    pub recovered_at_ms: u64,
    pub recovery_rto_ms: u64,
    pub sessions_before: u32,
    pub sessions_recovered: u32,
    pub economy_duplicate_count: u64,
    pub passed: bool,
    pub evidence_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeNetworkBetaRunPayload {
    pub schema: String,
    pub run_id: String,
    pub environment: HomeBetaEnvironment,
    pub node_id: String,
    pub node_public_key: String,
    pub key_generation: u64,
    pub provider_code: String,
    pub provider_asn: u32,
    pub failure_domain: String,
    pub coarse_region: String,
    pub cgnat_observed: bool,
    pub inbound_port_opened: bool,
    pub relay_ip_hidden: bool,
    pub started_at_ms: u64,
    pub finished_at_ms: u64,
    pub active_session_minutes: u64,
    pub maximum_failover_rto_ms: u64,
    pub economy_duplicate_count: u64,
    pub faults: Vec<HomeBetaFaultObservation>,
    pub build_commit: String,
    pub machine_attestation_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignedHomeNetworkBetaRun {
    pub payload: HomeNetworkBetaRunPayload,
    pub node_signature_algorithm: String,
    pub node_signature: String,
    pub operator_public_key: String,
    pub operator_signature_algorithm: String,
    pub operator_signature: String,
}

impl SignedHomeNetworkBetaRun {
    pub fn sign(
        payload: HomeNetworkBetaRunPayload,
        node: &NodeSigningIdentity,
        operator: &NodeSigningIdentity,
    ) -> Result<Self, String> {
        validate_beta_payload(&payload, false)?;
        if payload.node_id != node.node_id() || payload.node_public_key != node.public_key() {
            return Err("Beta run payload does not match node signing identity".to_string());
        }
        let node_signature = node.sign(&domain_json(BETA_NODE_DOMAIN, &payload)?);
        let mut run = Self {
            payload,
            node_signature_algorithm: HOME_SIGNATURE_ALGORITHM.to_string(),
            node_signature,
            operator_public_key: operator.public_key().to_string(),
            operator_signature_algorithm: HOME_SIGNATURE_ALGORITHM.to_string(),
            operator_signature: String::new(),
        };
        run.operator_signature = operator.sign(&run.operator_signing_bytes()?);
        Ok(run)
    }

    pub fn verify(
        &self,
        trusted_operator_public_key: &str,
        require_physical: bool,
    ) -> Result<(), String> {
        validate_beta_payload(&self.payload, require_physical)?;
        if self.node_signature_algorithm != HOME_SIGNATURE_ALGORITHM
            || self.operator_signature_algorithm != HOME_SIGNATURE_ALGORITHM
        {
            return Err("unsupported Home Beta signature algorithm".to_string());
        }
        if self.operator_public_key != trusted_operator_public_key {
            return Err("Beta run operator is not trusted".to_string());
        }
        verify_ed25519_signature(
            &self.payload.node_public_key,
            &domain_json(BETA_NODE_DOMAIN, &self.payload)?,
            &self.node_signature,
        )?;
        verify_ed25519_signature(
            &self.operator_public_key,
            &self.operator_signing_bytes()?,
            &self.operator_signature,
        )
    }

    fn operator_signing_bytes(&self) -> Result<Vec<u8>, String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct OperatorAttestation<'a> {
            payload: &'a HomeNetworkBetaRunPayload,
            node_signature_algorithm: &'a str,
            node_signature: &'a str,
            operator_public_key: &'a str,
            operator_signature_algorithm: &'a str,
        }
        domain_json(
            BETA_OPERATOR_DOMAIN,
            &OperatorAttestation {
                payload: &self.payload,
                node_signature_algorithm: &self.node_signature_algorithm,
                node_signature: &self.node_signature,
                operator_public_key: &self.operator_public_key,
                operator_signature_algorithm: &self.operator_signature_algorithm,
            },
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeNetworkBetaCohortAcceptance {
    pub schema: String,
    pub accepted: bool,
    pub physical_run_count: u32,
    pub distinct_node_count: u32,
    pub distinct_provider_count: u32,
    pub distinct_asn_count: u32,
    pub distinct_failure_domain_count: u32,
    pub maximum_observed_rto_ms: u64,
    pub economy_duplicate_count: u64,
    pub run_ids: Vec<String>,
    pub production_claim: String,
}

pub fn verify_home_network_beta_cohort(
    runs: &[SignedHomeNetworkBetaRun],
    trusted_operator_public_key: &str,
) -> Result<HomeNetworkBetaCohortAcceptance, String> {
    if runs.len() < 3 {
        return Err("production Home Beta requires at least three signed runs".to_string());
    }
    let mut run_ids = BTreeSet::new();
    let mut nodes = BTreeSet::new();
    let mut providers = BTreeSet::new();
    let mut asns = BTreeSet::new();
    let mut failure_domains = BTreeSet::new();
    let mut maximum_rto = 0_u64;
    let mut duplicates = 0_u64;
    for run in runs {
        run.verify(trusted_operator_public_key, true)?;
        if !run_ids.insert(run.payload.run_id.clone()) {
            return Err("production Home Beta contains a duplicate run id".to_string());
        }
        nodes.insert(run.payload.node_id.clone());
        providers.insert(run.payload.provider_code.clone());
        asns.insert(run.payload.provider_asn);
        failure_domains.insert(run.payload.failure_domain.clone());
        maximum_rto = maximum_rto.max(run.payload.maximum_failover_rto_ms);
        duplicates = duplicates.saturating_add(run.payload.economy_duplicate_count);
    }
    if nodes.len() < 3 || providers.len() < 3 || asns.len() < 3 || failure_domains.len() < 3 {
        return Err(
            "production Home Beta requires three distinct nodes, providers, ASNs, and failure domains"
                .to_string(),
        );
    }
    if duplicates != 0 || maximum_rto > HOME_BETA_MAXIMUM_FAILOVER_RTO_MS {
        return Err("production Home Beta violates economy or failover SLO".to_string());
    }
    Ok(HomeNetworkBetaCohortAcceptance {
        schema: HOME_BETA_COHORT_SCHEMA.to_string(),
        accepted: true,
        physical_run_count: runs.len().min(u32::MAX as usize) as u32,
        distinct_node_count: nodes.len().min(u32::MAX as usize) as u32,
        distinct_provider_count: providers.len().min(u32::MAX as usize) as u32,
        distinct_asn_count: asns.len().min(u32::MAX as usize) as u32,
        distinct_failure_domain_count: failure_domains.len().min(u32::MAX as usize) as u32,
        maximum_observed_rto_ms: maximum_rto,
        economy_duplicate_count: duplicates,
        run_ids: run_ids.into_iter().collect(),
        production_claim: "three-provider physical Home Network Beta cryptographically accepted"
            .to_string(),
    })
}

fn validate_telemetry_payload(payload: &HomeNodeTelemetryPayload) -> Result<(), String> {
    if payload.schema != HOME_TELEMETRY_SCHEMA {
        return Err("unsupported Home telemetry schema".to_string());
    }
    if payload.key_generation == 0
        || payload.sequence == 0
        || payload.window_started_at_ms >= payload.observed_at_ms
        || payload.capacity_certificate_expires_at_ms <= payload.observed_at_ms
        || payload.placement_generation == 0
        || payload.capacity_max_sessions == 0
        || payload.capacity_max_zones == 0
        || payload.active_sessions > payload.capacity_max_sessions
        || payload.active_zones > payload.capacity_max_zones
        || payload.packet_loss_bps > 10_000
        || payload.cpu_usage_bps > 10_000
        || payload.memory_usage_bps > 10_000
    {
        return Err("Home telemetry contains an invalid counter, time, or capacity".to_string());
    }
    if node_id_from_public_key(&payload.public_key)? != payload.node_id {
        return Err("Home telemetry node id does not match public key".to_string());
    }
    for (label, value, maximum) in [
        ("agent instance id", payload.agent_instance_id.as_str(), 128),
        ("coarse region", payload.coarse_region.as_str(), 64),
        ("provider code", payload.provider_code.as_str(), 64),
        (
            "capacity certificate id",
            payload.capacity_certificate_id.as_str(),
            255,
        ),
        ("game id", payload.game_id.as_str(), 128),
        ("agent version", payload.agent_version.as_str(), 64),
    ] {
        validate_privacy_label(label, value, maximum)?;
    }
    if payload.zone_ids.len() != usize::from(payload.active_zones) {
        return Err("Home telemetry Zone list does not match active Zone count".to_string());
    }
    let mut zones = BTreeSet::new();
    for zone in &payload.zone_ids {
        validate_privacy_label("Zone id", zone, 255)?;
        if !zones.insert(zone) {
            return Err("Home telemetry contains a duplicate Zone id".to_string());
        }
    }
    Ok(())
}

fn validate_beta_payload(
    payload: &HomeNetworkBetaRunPayload,
    require_physical: bool,
) -> Result<(), String> {
    if payload.schema != HOME_BETA_RUN_SCHEMA {
        return Err("unsupported Home Network Beta schema".to_string());
    }
    if require_physical && payload.environment != HomeBetaEnvironment::PhysicalHomeNetwork {
        return Err("production Home Beta rejects lab or simulated network evidence".to_string());
    }
    if node_id_from_public_key(&payload.node_public_key)? != payload.node_id {
        return Err("Home Beta node id does not match public key".to_string());
    }
    if payload.key_generation == 0
        || payload.provider_asn == 0
        || payload.finished_at_ms <= payload.started_at_ms
        || payload.finished_at_ms.saturating_sub(payload.started_at_ms)
            < HOME_BETA_MINIMUM_DURATION_MS
        || payload.active_session_minutes == 0
        || !payload.cgnat_observed
        || payload.inbound_port_opened
        || !payload.relay_ip_hidden
        || payload.economy_duplicate_count != 0
        || payload.maximum_failover_rto_ms > HOME_BETA_MAXIMUM_FAILOVER_RTO_MS
    {
        return Err("Home Beta run violates production duration, privacy, or SLO".to_string());
    }
    for (label, value, maximum) in [
        ("run id", payload.run_id.as_str(), 128),
        ("provider code", payload.provider_code.as_str(), 64),
        ("failure domain", payload.failure_domain.as_str(), 128),
        ("coarse region", payload.coarse_region.as_str(), 64),
        ("build commit", payload.build_commit.as_str(), 128),
    ] {
        validate_privacy_label(label, value, maximum)?;
    }
    validate_sha256("machine attestation", &payload.machine_attestation_sha256)?;
    let mut observed = BTreeSet::new();
    let mut maximum_rto = 0_u64;
    for fault in &payload.faults {
        if !observed.insert(fault.kind) {
            return Err("Home Beta run contains duplicate fault observations".to_string());
        }
        validate_sha256("fault evidence", &fault.evidence_sha256)?;
        if !fault.passed
            || fault.recovered_at_ms < fault.injected_at_ms
            || fault.recovered_at_ms.saturating_sub(fault.injected_at_ms) != fault.recovery_rto_ms
            || fault.recovery_rto_ms > HOME_BETA_MAXIMUM_FAILOVER_RTO_MS
            || fault.sessions_before == 0
            || fault.sessions_recovered != fault.sessions_before
            || fault.economy_duplicate_count != 0
        {
            return Err("Home Beta fault observation violates continuity or RTO".to_string());
        }
        maximum_rto = maximum_rto.max(fault.recovery_rto_ms);
    }
    if observed != HomeBetaFaultKind::required() {
        return Err(
            "Home Beta run does not contain the complete required fault matrix".to_string(),
        );
    }
    if maximum_rto != payload.maximum_failover_rto_ms {
        return Err("Home Beta maximum RTO does not match fault observations".to_string());
    }
    Ok(())
}

fn validate_privacy_label(label: &str, value: &str, maximum: usize) -> Result<(), String> {
    validate_label(label, value, maximum)?;
    let trimmed = value.trim().trim_matches(['[', ']']);
    if trimmed.parse::<IpAddr>().is_ok()
        || value.to_ascii_lowercase().contains("ip=")
        || value.to_ascii_lowercase().contains("address=")
    {
        return Err(format!("{label} must not contain a raw IP address"));
    }
    Ok(())
}

fn validate_label(label: &str, value: &str, maximum: usize) -> Result<(), String> {
    if value.trim().is_empty() || value.len() > maximum || value.chars().any(char::is_control) {
        return Err(format!("invalid {label}"));
    }
    Ok(())
}

fn validate_sha256(label: &str, value: &str) -> Result<(), String> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("{label} must be a SHA-256 hex digest"));
    }
    Ok(())
}

fn domain_json<T: Serialize>(domain: &[u8], value: &T) -> Result<Vec<u8>, String> {
    let encoded =
        serde_json::to_vec(value).map_err(|error| format!("encode signed payload: {error}"))?;
    let mut bytes = Vec::with_capacity(domain.len() + encoded.len());
    bytes.extend_from_slice(domain);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

fn hex_lower(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn telemetry(identity: &NodeSigningIdentity) -> HomeNodeTelemetryPayload {
        HomeNodeTelemetryPayload {
            schema: HOME_TELEMETRY_SCHEMA.to_string(),
            node_id: identity.node_id().to_string(),
            public_key: identity.public_key().to_string(),
            key_generation: 3,
            agent_instance_id: "agent-boot-7".to_string(),
            sequence: 1,
            window_started_at_ms: 1_000,
            observed_at_ms: 2_000,
            coarse_region: "hk-region".to_string(),
            provider_code: "isp-a".to_string(),
            relay_rtt_ms: 31,
            packet_loss_bps: 10,
            measured_upstream_kbps: 50_000,
            active_sessions: 10,
            active_zones: 1,
            zone_ids: vec!["mir2/map/0".to_string()],
            checkpoint_lag_ms: 20,
            cpu_usage_bps: 4_000,
            memory_usage_bps: 3_000,
            work_mode: HomeAgentWorkMode::Serving,
            capacity_certificate_id: "capacity-7".to_string(),
            capacity_certificate_expires_at_ms: 20_000,
            capacity_max_sessions: 32,
            capacity_max_zones: 4,
            finalized_control_height: 11,
            placement_generation: 3,
            game_id: "mir2".to_string(),
            reward_epoch: 9,
            verified_work_units: 5,
            session_milliseconds: 600_000,
            agent_version: "0.1.0".to_string(),
        }
    }

    fn receipt(identity: &NodeSigningIdentity) -> VerifiedWorkReceipt {
        VerifiedWorkReceipt {
            receipt_id: "receipt-1".to_string(),
            game_id: "mir2".to_string(),
            epoch: 9,
            zone_id: "mir2/map/0".to_string(),
            control_height: 11,
            placement_generation: 3,
            work_units: 5,
            availability_bps: 9_900,
            quorum_node_ids: vec![identity.node_id().to_string(), "peer-b".to_string()],
            execution_commitment: "ab".repeat(32),
            observed_at_ms: 1_500,
        }
    }

    fn policy() -> GameRewardPolicy {
        GameRewardPolicy {
            game_id: "mir2".to_string(),
            epoch: 9,
            reward_budget: 1_000,
            reward_per_work_unit: 10,
            max_reward_per_node: 100,
            minimum_availability_bps: 9_000,
            minimum_quorum: 2,
            settlement_coin_type: "0x2::sui::SUI".to_string(),
        }
    }

    fn beta_payload(
        identity: &NodeSigningIdentity,
        suffix: u8,
        environment: HomeBetaEnvironment,
    ) -> HomeNetworkBetaRunPayload {
        let start = 10_000_u64 + u64::from(suffix) * 10_000;
        let faults = HomeBetaFaultKind::required()
            .into_iter()
            .enumerate()
            .map(|(index, kind)| {
                let injected = start + index as u64 * 10_000;
                HomeBetaFaultObservation {
                    kind,
                    injected_at_ms: injected,
                    recovered_at_ms: injected + 1_000 + u64::from(suffix),
                    recovery_rto_ms: 1_000 + u64::from(suffix),
                    sessions_before: 10,
                    sessions_recovered: 10,
                    economy_duplicate_count: 0,
                    passed: true,
                    evidence_sha256: format!("{:02x}", suffix).repeat(32),
                }
            })
            .collect();
        HomeNetworkBetaRunPayload {
            schema: HOME_BETA_RUN_SCHEMA.to_string(),
            run_id: format!("physical-run-{suffix}"),
            environment,
            node_id: identity.node_id().to_string(),
            node_public_key: identity.public_key().to_string(),
            key_generation: 1,
            provider_code: format!("isp-{suffix}"),
            provider_asn: 64_500 + u32::from(suffix),
            failure_domain: format!("home-{suffix}"),
            coarse_region: format!("region-{suffix}"),
            cgnat_observed: true,
            inbound_port_opened: false,
            relay_ip_hidden: true,
            started_at_ms: start,
            finished_at_ms: start + HOME_BETA_MINIMUM_DURATION_MS,
            active_session_minutes: 150,
            maximum_failover_rto_ms: 1_000 + u64::from(suffix),
            economy_duplicate_count: 0,
            faults,
            build_commit: format!("commit-{suffix}"),
            machine_attestation_sha256: format!("{:02x}", suffix + 10).repeat(32),
        }
    }

    #[test]
    fn signed_telemetry_is_private_fresh_and_replay_protected() {
        let identity = NodeSigningIdentity::from_seed([21; 32]);
        let report = SignedHomeNodeTelemetry::sign(telemetry(&identity), &identity).unwrap();
        report.verify(2_100, 1_000).unwrap();
        let guard = HomeTelemetryReplayGuard::default();
        guard.accept(&report, 2_100, 1_000).unwrap();
        assert!(guard.accept(&report, 2_100, 1_000).is_err());
        assert!(!serde_json::to_string(&report.operator_view())
            .unwrap()
            .contains("192.168."));

        let mut raw_ip = telemetry(&identity);
        raw_ip.coarse_region = "203.0.113.9".to_string();
        assert!(SignedHomeNodeTelemetry::sign(raw_ip, &identity).is_err());
    }

    #[test]
    fn relay_pseudonym_rotates_and_never_contains_source_ip() {
        let source: IpAddr = "203.0.113.7".parse().unwrap();
        let first = relay_source_ip_pseudonym(&[7; 32], "2026-07-26", source).unwrap();
        let same = relay_source_ip_pseudonym(&[7; 32], "2026-07-26", source).unwrap();
        let rotated = relay_source_ip_pseudonym(&[8; 32], "2026-07-27", source).unwrap();
        assert_eq!(first, same);
        assert_ne!(first, rotated);
        assert!(!first.contains("203.0.113.7"));
        assert!(relay_source_ip_pseudonym(&[7; 8], "bad", source).is_err());
    }

    #[test]
    fn rewards_pay_only_when_signed_telemetry_matches_quorum_receipts() {
        let identity = NodeSigningIdentity::from_seed([22; 32]);
        let report = SignedHomeNodeTelemetry::sign(telemetry(&identity), &identity).unwrap();
        let receipt = receipt(&identity);
        let accepted =
            reconcile_home_node_reward(&report, &[receipt.clone()], &policy(), 2_100, 1_000)
                .unwrap();
        assert!(accepted.payable);
        assert_eq!(accepted.accepted_work_score, 4);
        assert_eq!(accepted.estimated_reward, 40);

        let mut mismatched = telemetry(&identity);
        mismatched.verified_work_units = 500;
        let mismatched = SignedHomeNodeTelemetry::sign(mismatched, &identity).unwrap();
        let rejected =
            reconcile_home_node_reward(&mismatched, &[receipt], &policy(), 2_100, 1_000).unwrap();
        assert!(!rejected.payable);
        assert_eq!(rejected.accepted_work_score, 0);
        assert_eq!(rejected.estimated_reward, 0);
    }

    #[test]
    fn public_view_is_aggregate_only() {
        let first_identity = NodeSigningIdentity::from_seed([23; 32]);
        let second_identity = NodeSigningIdentity::from_seed([24; 32]);
        let first =
            SignedHomeNodeTelemetry::sign(telemetry(&first_identity), &first_identity).unwrap();
        let mut second_payload = telemetry(&second_identity);
        second_payload.agent_instance_id = "agent-boot-8".to_string();
        second_payload.relay_rtt_ms = 50;
        let second = SignedHomeNodeTelemetry::sign(second_payload, &second_identity).unwrap();
        let mut tampered = second.clone();
        tampered.payload.active_sessions += 1;
        assert!(aggregate_public_telemetry(&[tampered], 1).is_err());
        let public = aggregate_public_telemetry(&[first, second], 3).unwrap();
        assert_eq!(public.online_nodes, 2);
        assert_eq!(public.availability_bps, 6_666);
        assert_eq!(public.p95_relay_rtt_ms, 50);
        assert!(public.ip_hidden_by_regional_relay);
        let json = serde_json::to_string(&public).unwrap();
        assert!(!json.contains(first_identity.node_id()));
        assert!(!json.contains("isp-a"));
    }

    #[test]
    fn telemetry_store_enforces_retention_and_deletion() {
        let identity = NodeSigningIdentity::from_seed([25; 32]);
        let report = SignedHomeNodeTelemetry::sign(telemetry(&identity), &identity).unwrap();
        let store = HomeTelemetryStore::new(1_000, 2_000).unwrap();
        store.ingest(report, 2_100).unwrap();
        assert!(store.operator_view(identity.node_id(), 3_900).is_some());
        assert_eq!(store.prune(4_001).unwrap(), 1);
        assert!(store.operator_view(identity.node_id(), 4_001).is_none());

        let mut next = telemetry(&identity);
        next.sequence = 2;
        next.observed_at_ms = 5_000;
        next.window_started_at_ms = 4_000;
        next.capacity_certificate_expires_at_ms = 10_000;
        store
            .ingest(
                SignedHomeNodeTelemetry::sign(next, &identity).unwrap(),
                5_100,
            )
            .unwrap();
        assert!(store.delete_node(identity.node_id()).unwrap());
        assert!(store.operator_view(identity.node_id(), 5_100).is_none());
    }

    #[test]
    fn production_cohort_requires_three_physical_distinct_double_signed_runs() {
        let operator = NodeSigningIdentity::from_seed([31; 32]);
        let nodes = [
            NodeSigningIdentity::from_seed([32; 32]),
            NodeSigningIdentity::from_seed([33; 32]),
            NodeSigningIdentity::from_seed([34; 32]),
        ];
        let runs = nodes
            .iter()
            .enumerate()
            .map(|(index, node)| {
                SignedHomeNetworkBetaRun::sign(
                    beta_payload(
                        node,
                        (index + 1) as u8,
                        HomeBetaEnvironment::PhysicalHomeNetwork,
                    ),
                    node,
                    &operator,
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let acceptance = verify_home_network_beta_cohort(&runs, operator.public_key()).unwrap();
        assert!(acceptance.accepted);
        assert_eq!(acceptance.distinct_provider_count, 3);

        let simulated = SignedHomeNetworkBetaRun::sign(
            beta_payload(&nodes[0], 7, HomeBetaEnvironment::SimulatedNetwork),
            &nodes[0],
            &operator,
        )
        .unwrap();
        let mut mixed = runs;
        mixed[0] = simulated;
        assert!(verify_home_network_beta_cohort(&mixed, operator.public_key()).is_err());
    }

    #[test]
    fn beta_signature_detects_tampering_and_slo_regression() {
        let operator = NodeSigningIdentity::from_seed([41; 32]);
        let node = NodeSigningIdentity::from_seed([42; 32]);
        let mut run = SignedHomeNetworkBetaRun::sign(
            beta_payload(&node, 1, HomeBetaEnvironment::PhysicalHomeNetwork),
            &node,
            &operator,
        )
        .unwrap();
        run.verify(operator.public_key(), true).unwrap();
        run.payload.provider_code = "tampered-provider".to_string();
        assert!(run.verify(operator.public_key(), true).is_err());

        let mut slow = beta_payload(&node, 1, HomeBetaEnvironment::PhysicalHomeNetwork);
        slow.maximum_failover_rto_ms = 5_000;
        assert!(SignedHomeNetworkBetaRun::sign(slow, &node, &operator).is_err());
    }
}
