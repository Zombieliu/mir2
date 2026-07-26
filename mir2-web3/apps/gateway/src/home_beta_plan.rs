use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    node_id_from_public_key, verify_ed25519_signature, HomeBetaFaultKind, HomeBetaFaultObservation,
    HomeNetworkBetaRunPayload, NodeSigningIdentity, HOME_BETA_MAXIMUM_FAILOVER_RTO_MS,
    HOME_BETA_MINIMUM_DURATION_MS, HOME_BETA_RUN_SCHEMA, HOME_SIGNATURE_ALGORITHM,
};

pub const HOME_BETA_PLAN_SCHEMA: &str = "obelisk.home-network-beta-plan.v1";
pub const HOME_BETA_JOURNAL_SCHEMA: &str = "obelisk.home-network-beta-journal.v1";
pub const HOME_BETA_PLAN_MAXIMUM_LIFETIME_MS: u64 = 24 * 60 * 60 * 1_000;

const HOME_BETA_PLAN_DOMAIN: &[u8] = b"obelisk.home-network-beta-plan.v1\0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HomeBetaActionExecution {
    PassiveObservation,
    LocalUserConfirmation,
    BoundedNetworkProbe,
    StandbyVerification,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeBetaPlanAction {
    pub sequence: u16,
    pub fault: HomeBetaFaultKind,
    pub execution: HomeBetaActionExecution,
    pub timeout_ms: u64,
    pub minimum_observation_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeBetaTestPlanPayload {
    pub schema: String,
    pub plan_id: String,
    pub node_id: String,
    pub node_public_key: String,
    pub key_generation: u64,
    pub build_commit: String,
    pub issued_at_ms: u64,
    pub not_before_ms: u64,
    pub expires_at_ms: u64,
    pub minimum_run_duration_ms: u64,
    pub maximum_failover_rto_ms: u64,
    pub actions: Vec<HomeBetaPlanAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignedHomeBetaTestPlan {
    pub payload: HomeBetaTestPlanPayload,
    pub operator_public_key: String,
    pub signature_algorithm: String,
    pub signature: String,
}

impl SignedHomeBetaTestPlan {
    pub fn sign(
        payload: HomeBetaTestPlanPayload,
        operator: &NodeSigningIdentity,
    ) -> Result<Self, String> {
        validate_plan_payload(&payload)?;
        let signature = operator.sign(&plan_signing_bytes(&payload)?);
        Ok(Self {
            payload,
            operator_public_key: operator.public_key().to_string(),
            signature_algorithm: HOME_SIGNATURE_ALGORITHM.to_string(),
            signature,
        })
    }

    pub fn verify(
        &self,
        trusted_operator_public_key: &str,
        expected_node_id: &str,
        expected_node_public_key: &str,
        expected_build_commit: &str,
        now_ms: u64,
    ) -> Result<(), String> {
        validate_plan_payload(&self.payload)?;
        if self.operator_public_key != trusted_operator_public_key {
            return Err("Home Beta plan operator is not trusted".to_string());
        }
        if self.signature_algorithm != HOME_SIGNATURE_ALGORITHM {
            return Err("unsupported Home Beta plan signature algorithm".to_string());
        }
        verify_ed25519_signature(
            &self.operator_public_key,
            &plan_signing_bytes(&self.payload)?,
            &self.signature,
        )?;
        if self.payload.node_id != expected_node_id
            || self.payload.node_public_key != expected_node_public_key
        {
            return Err("Home Beta plan is bound to a different node".to_string());
        }
        if self.payload.build_commit != expected_build_commit {
            return Err("Home Beta plan is bound to a different client build".to_string());
        }
        if now_ms < self.payload.not_before_ms || now_ms > self.payload.expires_at_ms {
            return Err("Home Beta plan is not currently valid".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeBetaRunJournal {
    pub schema: String,
    pub plan: SignedHomeBetaTestPlan,
    pub trusted_operator_public_key: String,
    pub started_at_ms: u64,
    pub active_action_started_at_ms: Option<u64>,
    pub observations: Vec<HomeBetaFaultObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomeBetaRunMetadata {
    pub provider_code: String,
    pub provider_asn: u32,
    pub failure_domain: String,
    pub coarse_region: String,
    pub active_session_minutes: u64,
    pub machine_attestation_sha256: String,
}

impl HomeBetaRunJournal {
    pub fn begin(
        plan: SignedHomeBetaTestPlan,
        trusted_operator_public_key: &str,
        node: &NodeSigningIdentity,
        build_commit: &str,
        now_ms: u64,
    ) -> Result<Self, String> {
        plan.verify(
            trusted_operator_public_key,
            node.node_id(),
            node.public_key(),
            build_commit,
            now_ms,
        )?;
        Ok(Self {
            schema: HOME_BETA_JOURNAL_SCHEMA.to_string(),
            plan,
            trusted_operator_public_key: trusted_operator_public_key.to_string(),
            started_at_ms: now_ms,
            active_action_started_at_ms: None,
            observations: Vec::new(),
        })
    }

    pub fn start_action(
        &mut self,
        node: &NodeSigningIdentity,
        build_commit: &str,
        now_ms: u64,
    ) -> Result<HomeBetaPlanAction, String> {
        self.verify_binding(node, build_commit, now_ms)?;
        if self.active_action_started_at_ms.is_some() {
            return Err("Home Beta action is already active".to_string());
        }
        let action = self
            .plan
            .payload
            .actions
            .get(self.observations.len())
            .cloned()
            .ok_or_else(|| "Home Beta plan has no remaining action".to_string())?;
        self.active_action_started_at_ms = Some(now_ms);
        Ok(action)
    }

    pub fn complete_action(
        &mut self,
        sessions_before: u32,
        sessions_recovered: u32,
        economy_duplicate_count: u64,
        evidence_sha256: String,
        node: &NodeSigningIdentity,
        build_commit: &str,
        now_ms: u64,
    ) -> Result<(), String> {
        self.verify_binding(node, build_commit, now_ms)?;
        let action = self
            .plan
            .payload
            .actions
            .get(self.observations.len())
            .ok_or_else(|| "Home Beta plan has no remaining action".to_string())?;
        let injected_at_ms = self.active_action_started_at_ms.ok_or_else(|| {
            "Home Beta action must be started locally before completion".to_string()
        })?;
        let observation = HomeBetaFaultObservation {
            kind: action.fault,
            injected_at_ms,
            recovered_at_ms: now_ms,
            recovery_rto_ms: now_ms.saturating_sub(injected_at_ms),
            sessions_before,
            sessions_recovered,
            economy_duplicate_count,
            passed: true,
            evidence_sha256,
        };
        validate_observation(&observation, action)?;
        self.observations.push(observation);
        self.active_action_started_at_ms = None;
        Ok(())
    }

    pub fn finish(
        self,
        node: &NodeSigningIdentity,
        build_commit: &str,
        finished_at_ms: u64,
        metadata: HomeBetaRunMetadata,
    ) -> Result<HomeNetworkBetaRunPayload, String> {
        self.verify_binding(node, build_commit, finished_at_ms)?;
        if self.observations.len() != self.plan.payload.actions.len() {
            return Err("Home Beta run has unfinished plan actions".to_string());
        }
        if self.active_action_started_at_ms.is_some() {
            return Err("Home Beta run has an active unfinished action".to_string());
        }
        if finished_at_ms.saturating_sub(self.started_at_ms)
            < self.plan.payload.minimum_run_duration_ms
        {
            return Err("Home Beta run ended before the signed minimum duration".to_string());
        }
        let maximum_failover_rto_ms = self
            .observations
            .iter()
            .map(|observation| observation.recovery_rto_ms)
            .max()
            .unwrap_or_default();
        let economy_duplicate_count = self
            .observations
            .iter()
            .map(|observation| observation.economy_duplicate_count)
            .sum();
        let payload = HomeNetworkBetaRunPayload {
            schema: HOME_BETA_RUN_SCHEMA.to_string(),
            run_id: self.plan.payload.plan_id.clone(),
            environment: crate::HomeBetaEnvironment::PhysicalHomeNetwork,
            node_id: node.node_id().to_string(),
            node_public_key: node.public_key().to_string(),
            key_generation: self.plan.payload.key_generation,
            provider_code: metadata.provider_code,
            provider_asn: metadata.provider_asn,
            failure_domain: metadata.failure_domain,
            coarse_region: metadata.coarse_region,
            cgnat_observed: true,
            inbound_port_opened: false,
            relay_ip_hidden: true,
            started_at_ms: self.started_at_ms,
            finished_at_ms,
            active_session_minutes: metadata.active_session_minutes,
            maximum_failover_rto_ms,
            economy_duplicate_count,
            faults: self.observations,
            build_commit: build_commit.to_string(),
            machine_attestation_sha256: metadata.machine_attestation_sha256,
        };
        crate::NodeSignedHomeNetworkBetaRun::sign(payload.clone(), node)?;
        Ok(payload)
    }

    fn verify_binding(
        &self,
        node: &NodeSigningIdentity,
        build_commit: &str,
        now_ms: u64,
    ) -> Result<(), String> {
        if self.schema != HOME_BETA_JOURNAL_SCHEMA {
            return Err("unsupported Home Beta journal schema".to_string());
        }
        self.plan.verify(
            &self.trusted_operator_public_key,
            node.node_id(),
            node.public_key(),
            build_commit,
            now_ms,
        )?;
        if self.started_at_ms < self.plan.payload.not_before_ms
            || self.started_at_ms > self.plan.payload.expires_at_ms
        {
            return Err("Home Beta journal start time is outside the plan window".to_string());
        }
        Ok(())
    }
}

fn validate_plan_payload(payload: &HomeBetaTestPlanPayload) -> Result<(), String> {
    if payload.schema != HOME_BETA_PLAN_SCHEMA {
        return Err("unsupported Home Beta plan schema".to_string());
    }
    validate_label("plan id", &payload.plan_id, 128)?;
    validate_label("build commit", &payload.build_commit, 128)?;
    if node_id_from_public_key(&payload.node_public_key)? != payload.node_id {
        return Err("Home Beta plan node id does not match public key".to_string());
    }
    if payload.key_generation == 0
        || payload.not_before_ms < payload.issued_at_ms
        || payload.expires_at_ms <= payload.not_before_ms
        || payload.expires_at_ms.saturating_sub(payload.issued_at_ms)
            > HOME_BETA_PLAN_MAXIMUM_LIFETIME_MS
        || payload.minimum_run_duration_ms < HOME_BETA_MINIMUM_DURATION_MS
        || payload.maximum_failover_rto_ms > HOME_BETA_MAXIMUM_FAILOVER_RTO_MS
    {
        return Err("Home Beta plan contains invalid time, key, or SLO bounds".to_string());
    }
    if payload.actions.len() != HomeBetaFaultKind::required().len() {
        return Err("Home Beta plan must contain the complete fixed action matrix".to_string());
    }
    let mut faults = BTreeSet::new();
    for (index, action) in payload.actions.iter().enumerate() {
        if action.sequence != (index + 1) as u16 || !faults.insert(action.fault) {
            return Err("Home Beta plan actions must be unique and sequential".to_string());
        }
        if action.timeout_ms == 0
            || action.timeout_ms > 10 * 60 * 1_000
            || action.minimum_observation_ms > action.timeout_ms
        {
            return Err("Home Beta plan action has invalid bounded timing".to_string());
        }
        if action.execution != required_execution(action.fault) {
            return Err("Home Beta plan action requests a forbidden execution mode".to_string());
        }
    }
    if faults != HomeBetaFaultKind::required() {
        return Err("Home Beta plan action matrix is incomplete".to_string());
    }
    Ok(())
}

fn validate_observation(
    observation: &HomeBetaFaultObservation,
    action: &HomeBetaPlanAction,
) -> Result<(), String> {
    if observation.recovered_at_ms < observation.injected_at_ms
        || observation.recovery_rto_ms
            != observation
                .recovered_at_ms
                .saturating_sub(observation.injected_at_ms)
        || observation.recovery_rto_ms > HOME_BETA_MAXIMUM_FAILOVER_RTO_MS
        || observation.recovery_rto_ms > action.timeout_ms
        || observation.recovery_rto_ms < action.minimum_observation_ms
        || !observation.passed
        || observation.sessions_before == 0
        || observation.sessions_recovered != observation.sessions_before
        || observation.economy_duplicate_count != 0
        || !is_sha256(&observation.evidence_sha256)
    {
        return Err("Home Beta observation violates the signed action bounds".to_string());
    }
    Ok(())
}

fn required_execution(fault: HomeBetaFaultKind) -> HomeBetaActionExecution {
    match fault {
        HomeBetaFaultKind::CgnatBaseline => HomeBetaActionExecution::PassiveObservation,
        HomeBetaFaultKind::DynamicIpChange
        | HomeBetaFaultKind::RouterRestart
        | HomeBetaFaultKind::HostSleepWake => HomeBetaActionExecution::LocalUserConfirmation,
        HomeBetaFaultKind::PacketLoss | HomeBetaFaultKind::BandwidthCongestion => {
            HomeBetaActionExecution::BoundedNetworkProbe
        }
        HomeBetaFaultKind::ActiveFailureStandbyTakeover => {
            HomeBetaActionExecution::StandbyVerification
        }
    }
}

fn plan_signing_bytes(payload: &HomeBetaTestPlanPayload) -> Result<Vec<u8>, String> {
    let mut bytes = HOME_BETA_PLAN_DOMAIN.to_vec();
    bytes.extend(
        serde_json::to_vec(payload)
            .map_err(|error| format!("serialize Home Beta plan: {error}"))?,
    );
    Ok(bytes)
}

fn validate_label(label: &str, value: &str, maximum: usize) -> Result<(), String> {
    if value.trim().is_empty() || value.len() > maximum || value.chars().any(char::is_control) {
        return Err(format!("invalid {label}"));
    }
    Ok(())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(node: &NodeSigningIdentity) -> HomeBetaTestPlanPayload {
        let actions = HomeBetaFaultKind::required()
            .into_iter()
            .enumerate()
            .map(|(index, fault)| HomeBetaPlanAction {
                sequence: (index + 1) as u16,
                fault,
                execution: required_execution(fault),
                timeout_ms: 30_000,
                minimum_observation_ms: 1_000,
            })
            .collect();
        HomeBetaTestPlanPayload {
            schema: HOME_BETA_PLAN_SCHEMA.to_string(),
            plan_id: "home-beta-plan-1".to_string(),
            node_id: node.node_id().to_string(),
            node_public_key: node.public_key().to_string(),
            key_generation: 1,
            build_commit: "abc123".to_string(),
            issued_at_ms: 1_000,
            not_before_ms: 2_000,
            expires_at_ms: 1_000 + HOME_BETA_PLAN_MAXIMUM_LIFETIME_MS,
            minimum_run_duration_ms: HOME_BETA_MINIMUM_DURATION_MS,
            maximum_failover_rto_ms: HOME_BETA_MAXIMUM_FAILOVER_RTO_MS,
            actions,
        }
    }

    #[test]
    fn signed_plan_is_bound_to_node_build_window_and_allowlist() {
        let operator = NodeSigningIdentity::from_seed([61; 32]);
        let node = NodeSigningIdentity::from_seed([62; 32]);
        let signed = SignedHomeBetaTestPlan::sign(plan(&node), &operator).unwrap();
        signed
            .verify(
                operator.public_key(),
                node.node_id(),
                node.public_key(),
                "abc123",
                3_000,
            )
            .unwrap();
        assert!(signed
            .verify(
                operator.public_key(),
                node.node_id(),
                node.public_key(),
                "other-build",
                3_000,
            )
            .is_err());

        let mut forbidden = plan(&node);
        forbidden.actions[1].execution = HomeBetaActionExecution::BoundedNetworkProbe;
        assert!(SignedHomeBetaTestPlan::sign(forbidden, &operator).is_err());
    }

    #[test]
    fn journal_requires_ordered_complete_evidence_before_finish() {
        let operator = NodeSigningIdentity::from_seed([63; 32]);
        let node = NodeSigningIdentity::from_seed([64; 32]);
        let signed = SignedHomeBetaTestPlan::sign(plan(&node), &operator).unwrap();
        let mut journal =
            HomeBetaRunJournal::begin(signed, operator.public_key(), &node, "abc123", 3_000)
                .unwrap();
        let first = journal.start_action(&node, "abc123", 4_000).unwrap().fault;
        assert_eq!(first, HomeBetaFaultKind::CgnatBaseline);
        journal
            .complete_action(2, 2, 0, "ab".repeat(32), &node, "abc123", 5_000)
            .unwrap();
        assert!(journal
            .clone()
            .finish(
                &node,
                "abc123",
                3_000 + HOME_BETA_MINIMUM_DURATION_MS,
                HomeBetaRunMetadata {
                    provider_code: "isp-a".to_string(),
                    provider_asn: 64_500,
                    failure_domain: "home-a".to_string(),
                    coarse_region: "hk".to_string(),
                    active_session_minutes: 15,
                    machine_attestation_sha256: "cd".repeat(32),
                },
            )
            .is_err());
    }
}
