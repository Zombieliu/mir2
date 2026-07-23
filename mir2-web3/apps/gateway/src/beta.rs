use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use mir2_protocol::{ClientPacket, ServerPacket};
use mir2_simulation::{ActiveSessionIdentity, WorldCommand, WorldCommandExecution, WorldSnapshot};
use serde::{Deserialize, Serialize};

use crate::{
    CommonwareControlLog, FinalizedControlBlock, FinalizedControlProjector, GameRewardPolicy,
    GatewayConfig, GuildNodeAdmission, GuildNodeCapability, GuildNodeSecurityRegistry,
    HostedZoneOwnerCommandClient, InMemoryZoneOwnerLeaseAuthority, MultiGameRewardLedger,
    ProjectedControlEffect, ReplicatedControlCommand, RewardSettlementBatch, SettlementStatus,
    SharedInProcessZoneRuntimeFactory, SharedZoneOwnerLeaseAuthority, SharedZoneOwnerRpcTransport,
    VerifiedGuildNode, VerifiedGuildZoneTransport, VerifiedWorkMeterContext, ZoneHostControlPlane,
    ZoneHostHeartbeat, ZoneHostLifecycle, ZoneHostRegistration, ZoneId, ZoneOwnerCommandRequest,
    ZoneOwnerLeaseAuthority, ZoneOwnerRpcTransport, ZoneRuntimeFactory,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BetaReadinessRequirements {
    pub required_zones: Vec<ZoneId>,
    pub minimum_active_hosts: usize,
    pub minimum_failure_domains: usize,
    pub minimum_guild_quorum: usize,
    pub minimum_finalized_control_height: u64,
    pub required_finalized_reward_batch: Option<String>,
}

impl BetaReadinessRequirements {
    pub fn validate(&self) -> Result<(), String> {
        if self.required_zones.is_empty() {
            return Err("production beta requires at least one Zone".to_string());
        }
        let mut zones = BTreeSet::new();
        if !self.required_zones.iter().all(|zone| zones.insert(zone)) {
            return Err("production beta contains duplicate required Zones".to_string());
        }
        if self.minimum_active_hosts == 0
            || self.minimum_failure_domains == 0
            || self.minimum_guild_quorum == 0
            || self.minimum_finalized_control_height == 0
        {
            return Err("production beta readiness minima must be positive".to_string());
        }
        if self.minimum_failure_domains > self.minimum_active_hosts {
            return Err("failure-domain minimum exceeds active-host minimum".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BetaReadinessCheck {
    pub name: String,
    pub ready: bool,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BetaReadinessReport {
    pub ready: bool,
    pub observed_at_ms: u64,
    pub finalized_control_height: u64,
    pub checks: Vec<BetaReadinessCheck>,
}

impl BetaReadinessReport {
    pub fn require_ready(&self) -> Result<(), String> {
        if self.ready {
            return Ok(());
        }
        Err(self
            .checks
            .iter()
            .filter(|check| !check.ready)
            .map(|check| format!("{}: {}", check.name, check.detail))
            .collect::<Vec<_>>()
            .join("; "))
    }

    pub fn prometheus(&self) -> String {
        let mut lines = vec![
            "# HELP obelisk_beta_ready Production beta readiness (1 ready, 0 blocked).".into(),
            "# TYPE obelisk_beta_ready gauge".into(),
            format!("obelisk_beta_ready {}", u8::from(self.ready)),
            "# HELP obelisk_beta_control_height Last projected Commonware control height.".into(),
            "# TYPE obelisk_beta_control_height gauge".into(),
            format!(
                "obelisk_beta_control_height {}",
                self.finalized_control_height
            ),
            "# HELP obelisk_beta_check Individual production beta readiness checks.".into(),
            "# TYPE obelisk_beta_check gauge".into(),
        ];
        lines.extend(self.checks.iter().map(|check| {
            format!(
                "obelisk_beta_check{{check=\"{}\"}} {}",
                check.name.replace(['\\', '"'], "_"),
                u8::from(check.ready)
            )
        }));
        lines.push(String::new());
        lines.join("\n")
    }
}

#[derive(Debug)]
pub struct ProductionBetaReadinessProbe {
    requirements: BetaReadinessRequirements,
    scheduler: Arc<ZoneHostControlPlane>,
    guild_security: Arc<GuildNodeSecurityRegistry>,
    reward_ledger: Arc<Mutex<MultiGameRewardLedger>>,
}

impl ProductionBetaReadinessProbe {
    pub fn new(
        requirements: BetaReadinessRequirements,
        scheduler: Arc<ZoneHostControlPlane>,
        guild_security: Arc<GuildNodeSecurityRegistry>,
        reward_ledger: Arc<Mutex<MultiGameRewardLedger>>,
    ) -> Result<Self, String> {
        requirements.validate()?;
        Ok(Self {
            requirements,
            scheduler,
            guild_security,
            reward_ledger,
        })
    }

    pub fn evaluate(&self, now_ms: u64, finalized_control_height: u64) -> BetaReadinessReport {
        let hosts = self.scheduler.hosts(now_ms);
        let healthy_active = hosts
            .iter()
            .filter(|host| host.healthy && host.lifecycle == ZoneHostLifecycle::Active)
            .collect::<Vec<_>>();
        let failure_domains = healthy_active
            .iter()
            .map(|host| host.registration.failure_domain.as_str())
            .collect::<BTreeSet<_>>();
        let healthy_host_ids = healthy_active
            .iter()
            .map(|host| host.registration.host_id.as_str())
            .collect::<BTreeSet<_>>();
        let placements_ready = self.requirements.required_zones.iter().all(|zone| {
            self.scheduler.placement(zone).is_some_and(|placement| {
                placement.expires_at_ms > now_ms
                    && placement
                        .host_ids()
                        .all(|host_id| healthy_host_ids.contains(host_id))
            })
        });
        let guild_ready = self
            .guild_security
            .snapshots()
            .iter()
            .filter(|snapshot| {
                self.guild_security.is_eligible(
                    &snapshot.admission.node_id,
                    GuildNodeCapability::ExecuteZone,
                    now_ms,
                )
            })
            .count();
        let settlement_ready = match self.requirements.required_finalized_reward_batch.as_deref() {
            None => true,
            Some(batch_id) => self
                .reward_ledger
                .lock()
                .ok()
                .and_then(|ledger| ledger.settlement_status(batch_id).cloned())
                .is_some_and(|status| matches!(status, SettlementStatus::Finalized { .. })),
        };
        let checks = vec![
            BetaReadinessCheck {
                name: "commonware_finality".to_string(),
                ready: finalized_control_height
                    >= self.requirements.minimum_finalized_control_height,
                detail: format!(
                    "height {finalized_control_height}, minimum {}",
                    self.requirements.minimum_finalized_control_height
                ),
            },
            BetaReadinessCheck {
                name: "zone_hosts".to_string(),
                ready: healthy_active.len() >= self.requirements.minimum_active_hosts,
                detail: format!(
                    "{} healthy active hosts, minimum {}",
                    healthy_active.len(),
                    self.requirements.minimum_active_hosts
                ),
            },
            BetaReadinessCheck {
                name: "failure_domains".to_string(),
                ready: failure_domains.len() >= self.requirements.minimum_failure_domains,
                detail: format!(
                    "{} healthy failure domains, minimum {}",
                    failure_domains.len(),
                    self.requirements.minimum_failure_domains
                ),
            },
            BetaReadinessCheck {
                name: "zone_placements".to_string(),
                ready: placements_ready,
                detail: format!(
                    "{} required placements are leased only to healthy hosts",
                    self.requirements.required_zones.len()
                ),
            },
            BetaReadinessCheck {
                name: "guild_execution_quorum".to_string(),
                ready: guild_ready >= self.requirements.minimum_guild_quorum,
                detail: format!(
                    "{guild_ready} admitted executors, minimum {}",
                    self.requirements.minimum_guild_quorum
                ),
            },
            BetaReadinessCheck {
                name: "reward_settlement".to_string(),
                ready: settlement_ready,
                detail: self
                    .requirements
                    .required_finalized_reward_batch
                    .as_ref()
                    .map(|batch| format!("required finalized batch {batch}"))
                    .unwrap_or_else(|| "no settlement batch required at boot".to_string()),
            },
        ];
        BetaReadinessReport {
            ready: checks.iter().all(|check| check.ready),
            observed_at_ms: now_ms,
            finalized_control_height,
            checks,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Gate10AcceptanceEvidence {
    pub report: BetaReadinessReport,
    pub verified_receipt_count: usize,
    pub quarantined_node_count: usize,
    pub reward_batch: RewardSettlementBatch,
    pub sui_transaction_digest: String,
    pub sui_checkpoint: u64,
}

#[derive(Debug)]
struct AcceptanceDivergentTransport {
    inner: SharedZoneOwnerRpcTransport,
}

impl ZoneOwnerRpcTransport for AcceptanceDivergentTransport {
    fn execute(&self, request: ZoneOwnerCommandRequest) -> Result<WorldCommandExecution, String> {
        let mut execution = self.inner.execute(request)?;
        execution
            .packets
            .push(ServerPacket::KeepAlive { time: i64::MAX });
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

/// Deterministic, no-secret Gate 10 acceptance path spanning Gates 6 through 9.
pub fn run_gate10_acceptance() -> Result<Gate10AcceptanceEvidence, String> {
    const NOW_MS: u64 = 1_000;
    let committee = ["validator-a", "validator-b", "validator-c", "validator-d"]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let log = CommonwareControlLog::new(committee)?;
    let scheduler = Arc::new(ZoneHostControlPlane::new(10_000, 30_000, 1));
    let guild_security = Arc::new(GuildNodeSecurityRegistry::new(1, 60_000));
    let reward_ledger = Arc::new(Mutex::new(MultiGameRewardLedger::default()));
    let projector = FinalizedControlProjector::new(scheduler.clone(), guild_security.clone())
        .with_reward_ledger(reward_ledger.clone());
    let zone_id = ZoneId::new("mir2/map/0");

    let mut bootstrap = Vec::new();
    for (host_id, port, domain) in [
        ("zone-a", 7301, "az-a"),
        ("zone-b", 7302, "az-b"),
        ("zone-c", 7303, "az-c"),
    ] {
        bootstrap.push(
            ReplicatedControlCommand::RegisterZoneHost {
                registration: ZoneHostRegistration {
                    host_id: host_id.to_string(),
                    endpoint: format!("127.0.0.1:{port}"),
                    failure_domain: domain.to_string(),
                    max_sessions: 1_000,
                    max_sessions_per_zone: 100,
                    max_zones: 100,
                    weight: 100,
                },
                heartbeat: ZoneHostHeartbeat {
                    session_count: 0,
                    busiest_zone_session_count: 0,
                    active_connections: 0,
                    observed_at_ms: NOW_MS,
                },
            }
            .envelope(format!("register-{host_id}"))?,
        );
    }
    bootstrap.push(
        ReplicatedControlCommand::PlaceZone {
            zone_id: zone_id.clone(),
            now_ms: NOW_MS,
        }
        .envelope("place-mir2-map-0")?,
    );
    for node_id in ["guild-a", "guild-b", "guild-divergent"] {
        bootstrap.push(
            ReplicatedControlCommand::AdmitGuildNode {
                admission: GuildNodeAdmission::zone_executor(node_id, "acceptance-guild", u64::MAX),
                now_ms: NOW_MS,
            }
            .envelope(format!("admit-{node_id}"))?,
        );
    }
    bootstrap.push(
        ReplicatedControlCommand::RegisterGameRewardPolicy {
            policy: GameRewardPolicy {
                game_id: "mir2".to_string(),
                epoch: 1,
                reward_budget: 1_000_000,
                reward_per_work_unit: 100,
                max_reward_per_node: 500_000,
                minimum_availability_bps: 9_000,
                minimum_quorum: 2,
                settlement_coin_type: "0x2::sui::SUI".to_string(),
            },
        }
        .envelope("mir2-reward-policy-1")?,
    );
    let bootstrap = finalize(&log, "validator-a", bootstrap)?;
    projector.apply(&bootstrap)?;
    let placement = scheduler
        .placement(&zone_id)
        .ok_or_else(|| "acceptance placement missing after finality".to_string())?;

    let lease_authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let shared_authority: SharedZoneOwnerLeaseAuthority = lease_authority.clone();
    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut nodes = ["guild-a", "guild-b"]
        .into_iter()
        .map(|node_id| {
            let runtime = factory.create_runtime(GatewayConfig::default(), &zone_id);
            VerifiedGuildNode {
                node_id: node_id.to_string(),
                transport: Arc::new(HostedZoneOwnerCommandClient::with_owner_lease_authority(
                    runtime,
                    shared_authority.clone(),
                )),
            }
        })
        .collect::<Vec<_>>();
    let divergent_runtime = factory.create_runtime(GatewayConfig::default(), &zone_id);
    let divergent_inner: SharedZoneOwnerRpcTransport =
        Arc::new(HostedZoneOwnerCommandClient::with_owner_lease_authority(
            divergent_runtime,
            shared_authority,
        ));
    nodes.push(VerifiedGuildNode {
        node_id: "guild-divergent".to_string(),
        transport: Arc::new(AcceptanceDivergentTransport {
            inner: divergent_inner,
        }),
    });
    let verified = VerifiedGuildZoneTransport::new(nodes, 2, guild_security.clone())?
        .with_work_meter(VerifiedWorkMeterContext {
            game_id: "mir2".to_string(),
            epoch: 1,
            finalized_control_height: bootstrap.block.height,
            placement_generation: placement.generation,
            availability_bps: 10_000,
        })?;
    verified.execute(ZoneOwnerCommandRequest::direct(
        lease_authority.owner_lease(&zone_id),
        WorldCommand::ClientPacket(ClientPacket::KeepAlive { time: 42 }),
    ))?;
    let receipts = verified.drain_verified_work_receipts();
    let quarantined_node_count = guild_security
        .snapshots()
        .iter()
        .filter(|snapshot| snapshot.quarantine_until_ms > 0)
        .count();
    if quarantined_node_count != 1 {
        return Err(format!(
            "acceptance expected one divergent node quarantine, got {quarantined_node_count}"
        ));
    }
    for receipt in receipts.iter().cloned() {
        reward_ledger
            .lock()
            .map_err(|_| "acceptance reward ledger mutex poisoned".to_string())?
            .ingest_verified(receipt)?;
    }

    let close = finalize(
        &log,
        "validator-b",
        vec![ReplicatedControlCommand::FinalizeGameRewardEpoch {
            game_id: "mir2".to_string(),
            epoch: 1,
        }
        .envelope("mir2-reward-close-1")?],
    )?;
    let reward_batch = projector
        .apply(&close)?
        .into_iter()
        .find_map(|effect| match effect {
            ProjectedControlEffect::RewardEpochFinalized(batch) => Some(batch),
            _ => None,
        })
        .ok_or_else(|| "acceptance reward epoch did not finalize".to_string())?;
    let transaction_digest = "acceptance-sui-tx".to_string();
    let checkpoint = 9_001;
    reward_ledger
        .lock()
        .map_err(|_| "acceptance reward ledger mutex poisoned".to_string())?
        .mark_finalized(&reward_batch.batch_id, &transaction_digest, checkpoint)?;

    let requirements = BetaReadinessRequirements {
        required_zones: vec![zone_id],
        minimum_active_hosts: 3,
        minimum_failure_domains: 3,
        minimum_guild_quorum: 2,
        minimum_finalized_control_height: 2,
        required_finalized_reward_batch: Some(reward_batch.batch_id.clone()),
    };
    let report =
        ProductionBetaReadinessProbe::new(requirements, scheduler, guild_security, reward_ledger)?
            .evaluate(NOW_MS + 1, projector.last_height());
    report.require_ready()?;
    Ok(Gate10AcceptanceEvidence {
        report,
        verified_receipt_count: receipts.len(),
        quarantined_node_count,
        reward_batch,
        sui_transaction_digest: transaction_digest,
        sui_checkpoint: checkpoint,
    })
}

fn finalize(
    log: &CommonwareControlLog,
    proposer: &str,
    commands: Vec<crate::ControlCommandEnvelope>,
) -> Result<FinalizedControlBlock, String> {
    let block = log.propose(proposer, commands)?;
    for validator in ["validator-a", "validator-b"] {
        if log.vote(validator, &block.digest)?.is_some() {
            return Err("acceptance block finalized below expected quorum".to_string());
        }
    }
    log.vote("validator-c", &block.digest)?
        .ok_or_else(|| "acceptance block did not reach Commonware quorum".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readiness_fails_closed_without_finality_capacity_or_quorum() {
        let probe = ProductionBetaReadinessProbe::new(
            BetaReadinessRequirements {
                required_zones: vec![ZoneId::new("mir2/map/0")],
                minimum_active_hosts: 2,
                minimum_failure_domains: 2,
                minimum_guild_quorum: 2,
                minimum_finalized_control_height: 1,
                required_finalized_reward_batch: Some("missing-batch".to_string()),
            },
            Arc::new(ZoneHostControlPlane::new(100, 1_000, 1)),
            Arc::new(GuildNodeSecurityRegistry::default()),
            Arc::new(Mutex::new(MultiGameRewardLedger::default())),
        )
        .unwrap();
        let report = probe.evaluate(100, 0);
        assert!(!report.ready);
        assert_eq!(report.checks.iter().filter(|check| !check.ready).count(), 6);
        assert!(report
            .require_ready()
            .unwrap_err()
            .contains("commonware_finality"));
        assert!(report.prometheus().contains("obelisk_beta_ready 0"));
    }

    #[test]
    fn gate10_acceptance_closes_verified_work_to_finalized_sui_settlement() {
        let evidence = run_gate10_acceptance().unwrap();
        assert!(evidence.report.ready);
        assert_eq!(evidence.report.finalized_control_height, 2);
        assert_eq!(evidence.verified_receipt_count, 1);
        assert_eq!(evidence.quarantined_node_count, 1);
        assert_eq!(evidence.reward_batch.game_id, "mir2");
        assert_eq!(evidence.reward_batch.allocation_count, 2);
        assert!(evidence
            .report
            .prometheus()
            .contains("obelisk_beta_ready 1"));
    }
}
