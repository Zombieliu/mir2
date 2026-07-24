//! Machine-readable Gate 18-21 Regional acceptance profile.
//!
//! Capacity results are only comparable when player behaviour, fault
//! injection, SLOs, and deployment resources are fixed. This module validates
//! that contract before a certification workload is allowed to start.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

pub const REGIONAL_PROFILE_SCHEMA_VERSION: u32 = 1;
pub const REGIONAL_REFERENCE_PROFILE_ID: &str = "mir2-regional-v1";
pub const REGIONAL_REFERENCE_PROFILE_JSON: &str =
    include_str!("../../../infra/regional/profile.json");

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegionalProfile {
    pub schema_version: u32,
    pub profile_id: String,
    pub catalog_maps: usize,
    pub active_maps: usize,
    pub stages: RegionalStages,
    pub workload: RegionalWorkload,
    pub faults: Vec<String>,
    pub reference_deployment: RegionalReferenceDeployment,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegionalStages {
    pub gate18: RegionalStage,
    pub gate19: RegionalStage,
    pub gate20: RegionalStage,
    pub gate21: RegionalStage,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegionalStage {
    pub concurrent_players: usize,
    pub duration_seconds: u64,
    #[serde(default)]
    pub hot_map_players: Option<usize>,
    #[serde(default)]
    pub maximum_command_p95_ms: Option<f64>,
    #[serde(default)]
    pub maximum_command_p99_ms: Option<f64>,
    pub maximum_error_rate: f64,
    #[serde(default)]
    pub maximum_zone_failover_rto_ms: Option<u64>,
    #[serde(default)]
    pub maximum_gateway_reconnect_rto_ms: Option<u64>,
    #[serde(default)]
    pub minimum_single_fault_scenarios: Option<usize>,
    pub economy_duplicate_count: u64,
    #[serde(default)]
    pub negative_economy_balance_count: Option<u64>,
    #[serde(default)]
    pub orphan_economy_transaction_count: Option<u64>,
    #[serde(default)]
    pub maximum_sustained_memory_growth_percent: Option<f64>,
    #[serde(default)]
    pub maximum_sustained_wal_growth_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegionalWorkload {
    pub movement_players_percent: u8,
    pub combat_players_percent: u8,
    pub social_economy_players_percent: u8,
    pub idle_players_percent: u8,
    pub movement_commands_per_second: u32,
    pub combat_commands_per_second: u32,
    pub social_economy_transactions_per_minute: u32,
    pub idle_keep_alive_seconds: u64,
    pub minimum_distinct_accounts_percent: u8,
    pub minimum_distinct_characters_percent: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegionalReferenceDeployment {
    pub gateway_replicas: usize,
    pub zone_host_replicas: usize,
    pub commonware_validators: usize,
    pub postgres_replicas: usize,
    pub redis_replicas: usize,
    pub gateway_cpu_per_replica: usize,
    pub gateway_memory_gi_b_per_replica: usize,
    pub zone_host_cpu_per_replica: usize,
    pub zone_host_memory_gi_b_per_replica: usize,
    pub postgres_cpu_per_replica: usize,
    pub postgres_memory_gi_b_per_replica: usize,
    pub redis_cpu_per_replica: usize,
    pub redis_memory_gi_b_per_replica: usize,
}

impl RegionalProfile {
    pub fn reference() -> Result<Self, String> {
        Self::from_json(REGIONAL_REFERENCE_PROFILE_JSON.as_bytes())
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let bytes = fs::read(path).map_err(|error| {
            format!(
                "failed to read Regional profile {}: {error}",
                path.display()
            )
        })?;
        Self::from_json(&bytes)
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self, String> {
        let profile: Self = serde_json::from_slice(bytes)
            .map_err(|error| format!("failed to decode Regional profile: {error}"))?;
        profile.validate()?;
        Ok(profile)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != REGIONAL_PROFILE_SCHEMA_VERSION {
            return Err(format!(
                "unsupported Regional profile schema {}, expected {}",
                self.schema_version, REGIONAL_PROFILE_SCHEMA_VERSION
            ));
        }
        validate_identifier("profile id", &self.profile_id)?;
        if self.catalog_maps == 0 || self.active_maps == 0 || self.active_maps > self.catalog_maps {
            return Err("Regional map counts must satisfy 0 < active <= catalog".to_string());
        }

        let percentages = [
            self.workload.movement_players_percent,
            self.workload.combat_players_percent,
            self.workload.social_economy_players_percent,
            self.workload.idle_players_percent,
        ];
        if percentages
            .iter()
            .map(|value| u16::from(*value))
            .sum::<u16>()
            != 100
        {
            return Err("Regional workload player percentages must total 100".to_string());
        }
        if self.workload.minimum_distinct_accounts_percent != 100
            || self.workload.minimum_distinct_characters_percent != 100
        {
            return Err(
                "Regional certification requires distinct accounts and characters".to_string(),
            );
        }
        if self.workload.movement_commands_per_second == 0
            || self.workload.combat_commands_per_second == 0
            || self.workload.social_economy_transactions_per_minute == 0
            || self.workload.idle_keep_alive_seconds == 0
        {
            return Err("Regional workload rates must be positive".to_string());
        }

        let stages = [
            ("gate18", &self.stages.gate18),
            ("gate19", &self.stages.gate19),
            ("gate20", &self.stages.gate20),
            ("gate21", &self.stages.gate21),
        ];
        let mut previous_players = 0;
        let mut previous_duration = 0;
        for (name, stage) in stages {
            stage.validate(name)?;
            if stage.concurrent_players < previous_players {
                return Err(format!(
                    "{name} concurrent players must not decrease from the previous Gate"
                ));
            }
            if stage.duration_seconds < previous_duration {
                return Err(format!(
                    "{name} duration must not decrease from the previous Gate"
                ));
            }
            previous_players = stage.concurrent_players;
            previous_duration = stage.duration_seconds;
        }

        let unique_faults = self
            .faults
            .iter()
            .map(|fault| fault.trim())
            .collect::<BTreeSet<_>>();
        if unique_faults.len() != self.faults.len()
            || unique_faults.len() < 6
            || unique_faults.iter().any(|fault| fault.is_empty())
        {
            return Err(
                "Regional profile requires at least six distinct non-empty faults".to_string(),
            );
        }
        self.reference_deployment.validate()?;
        Ok(())
    }

    pub fn require_reference_contract(&self) -> Result<(), String> {
        self.validate()?;
        let reference = Self::reference()?;
        if self != &reference {
            return Err(format!(
                "Regional evidence must use the exact {REGIONAL_REFERENCE_PROFILE_ID} contract"
            ));
        }
        Ok(())
    }
}

impl RegionalStage {
    fn validate(&self, name: &str) -> Result<(), String> {
        if self.concurrent_players == 0 || self.duration_seconds == 0 {
            return Err(format!("{name} players and duration must be positive"));
        }
        if !(0.0..1.0).contains(&self.maximum_error_rate) {
            return Err(format!("{name} maximum error rate must be in [0,1)"));
        }
        if self.economy_duplicate_count != 0 {
            return Err(format!("{name} cannot permit duplicate economy results"));
        }
        if let Some(hot_map_players) = self.hot_map_players {
            if hot_map_players == 0 || hot_map_players > self.concurrent_players {
                return Err(format!("{name} hot-map player count is invalid"));
            }
        }
        for (label, value) in [
            ("p95", self.maximum_command_p95_ms),
            ("p99", self.maximum_command_p99_ms),
            (
                "memory growth",
                self.maximum_sustained_memory_growth_percent,
            ),
        ] {
            if value.is_some_and(|value| !value.is_finite() || value <= 0.0) {
                return Err(format!(
                    "{name} maximum {label} must be finite and positive"
                ));
            }
        }
        if let (Some(p95), Some(p99)) = (self.maximum_command_p95_ms, self.maximum_command_p99_ms) {
            if p99 < p95 {
                return Err(format!("{name} p99 threshold cannot be lower than p95"));
            }
        }
        Ok(())
    }
}

impl RegionalReferenceDeployment {
    fn validate(&self) -> Result<(), String> {
        if self.gateway_replicas < 3
            || self.zone_host_replicas < 2
            || self.commonware_validators < 4
            || self.postgres_replicas < 2
            || self.redis_replicas < 3
        {
            return Err(
                "Regional reference deployment has an infrastructure single point of failure"
                    .to_string(),
            );
        }
        let resources = [
            self.gateway_cpu_per_replica,
            self.gateway_memory_gi_b_per_replica,
            self.zone_host_cpu_per_replica,
            self.zone_host_memory_gi_b_per_replica,
            self.postgres_cpu_per_replica,
            self.postgres_memory_gi_b_per_replica,
            self.redis_cpu_per_replica,
            self.redis_memory_gi_b_per_replica,
        ];
        if resources.contains(&0) {
            return Err("Regional reference deployment resources must be positive".to_string());
        }
        Ok(())
    }
}

fn validate_identifier(label: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty()
        || value.len() > 128
        || value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err(format!("invalid Regional {label}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_profile_is_valid_and_exact() {
        let profile = RegionalProfile::reference().expect("reference profile");
        profile
            .require_reference_contract()
            .expect("exact reference contract");
        assert_eq!(profile.stages.gate18.concurrent_players, 500);
        assert_eq!(profile.stages.gate20.concurrent_players, 1_000);
        assert_eq!(profile.stages.gate21.concurrent_players, 3_000);
        assert_eq!(profile.stages.gate21.duration_seconds, 72 * 60 * 60);
    }

    #[test]
    fn workload_cannot_count_only_idle_connections() {
        let mut profile = RegionalProfile::reference().expect("reference profile");
        profile.workload.idle_players_percent = 100;
        assert!(profile
            .validate()
            .expect_err("invalid percentages")
            .contains("percentages"));
    }

    #[test]
    fn stage_cannot_relax_economy_or_latency_invariants() {
        let mut profile = RegionalProfile::reference().expect("reference profile");
        profile.stages.gate21.economy_duplicate_count = 1;
        assert!(profile
            .validate()
            .expect_err("duplicate economy")
            .contains("duplicate economy"));

        let mut profile = RegionalProfile::reference().expect("reference profile");
        profile.stages.gate21.maximum_command_p99_ms = Some(100.0);
        assert!(profile
            .validate()
            .expect_err("p99 below p95")
            .contains("p99"));
    }
}
