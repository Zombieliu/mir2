use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use mir2_game_data::{crystal_map_respawns_by_file_name, crystal_monster_by_name};
use mir2_simulation::{
    crystal_world_respawn_spawns, ZoneMonsterDefense, ZoneMonsterSpawn, ZoneNativeMonsterSnapshot,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::consensus_log::{CommonwareControlLog, ControlCommandEnvelope, FinalizedControlBlock};
use crate::node_identity::{verify_ed25519_signature, NodeSigningIdentity};
use crate::routing::{SharedInProcessZoneRuntimeFactory, ZoneId};

pub const WORLD_DIRECTOR_SCHEMA: &str = "obelisk.world-director.v1";
pub const WORLD_DIRECTOR_NAMESPACE: &str = "obelisk.world-director.v1";
pub const MAX_AI_PROPOSAL_BYTES: usize = 16 * 1024;
const WORLD_DIRECTOR_COMMAND_DOMAIN: &[u8] = b"obelisk.world-director.command.v1\0";
const WORLD_DIRECTOR_RECEIPT_DOMAIN: &[u8] = b"obelisk.world-director.receipt.v1\0";
const WORLD_DIRECTOR_FINALITY_DOMAIN: &[u8] = b"obelisk.world-director.finality.v1\0";
const DIRECTOR_SIMULATION_CHECKPOINT_VERSION: u32 = 1;
const DIRECTOR_SIMULATION_CHECKPOINT_DOMAIN: &[u8] =
    b"obelisk.world-director.simulation-checkpoint.v1\0";
const DIRECTOR_RUNTIME_CHECKPOINT_VERSION: u32 = 3;
const DIRECTOR_RUNTIME_CHECKPOINT_DOMAIN: &[u8] = b"obelisk.world-director.runtime-checkpoint.v1\0";
const DIRECTOR_RUNTIME_CHECKPOINT_INTERVAL_MS: u64 = 30_000;
const MAX_DIRECTOR_RUNTIME_CHECKPOINT_BYTES: u64 = 256 * 1024 * 1024;
static DIRECTOR_CHECKPOINT_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);
const MAX_DIRECTOR_TEXT_BYTES: usize = 512;
const BASIS_POINTS: u64 = 10_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapTelemetrySnapshot {
    pub zone_id: String,
    pub active_players: u32,
    pub median_level: u16,
    pub new_player_count: u32,
    pub returning_player_count: u32,
    pub monster_kills: u64,
    pub boss_kills: u64,
    pub player_deaths: u64,
    pub completed_quests: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EconomyTelemetrySnapshot {
    pub gold_created: u64,
    pub gold_destroyed: u64,
    pub median_trade_price_index_bps: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildTelemetrySnapshot {
    pub active_guilds: u32,
    pub largest_guild_population_bps: u16,
    pub largest_guild_boss_kill_share_bps: u16,
}

/// A bounded, aggregate-only view of the game world. Account identifiers,
/// chat text, IP addresses and exact inventories deliberately do not belong in
/// the director input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldTelemetrySnapshot {
    pub schema: String,
    pub snapshot_id: String,
    pub game_id: String,
    pub region_id: String,
    pub observed_at_ms: u64,
    pub window_ms: u64,
    pub maps: Vec<MapTelemetrySnapshot>,
    pub economy: EconomyTelemetrySnapshot,
    pub guilds: GuildTelemetrySnapshot,
}

impl WorldTelemetrySnapshot {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != WORLD_DIRECTOR_SCHEMA {
            return Err("unsupported world telemetry schema".to_string());
        }
        validate_component("snapshot id", &self.snapshot_id)?;
        validate_component("game id", &self.game_id)?;
        validate_component("region id", &self.region_id)?;
        if self.observed_at_ms == 0 || self.window_ms == 0 {
            return Err("world telemetry timestamps must be positive".to_string());
        }
        if self.maps.is_empty() || self.maps.len() > 1_024 {
            return Err("world telemetry must contain 1..=1024 maps".to_string());
        }
        let mut zones = BTreeSet::new();
        for map in &self.maps {
            validate_component("telemetry zone id", &map.zone_id)?;
            if !zones.insert(map.zone_id.as_str()) {
                return Err(format!("duplicate telemetry zone {}", map.zone_id));
            }
        }
        if self.economy.median_trade_price_index_bps == 0
            || self.guilds.largest_guild_population_bps > BASIS_POINTS as u16
            || self.guilds.largest_guild_boss_kill_share_bps > BASIS_POINTS as u16
        {
            return Err("world telemetry contains an invalid basis-point value".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DirectorPressure {
    PopulationImbalance,
    ContentFatigue,
    ProgressionGap,
    EconomyInflation,
    GuildDominance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectorPressureScores {
    pub population_imbalance_bps: u16,
    pub content_fatigue_bps: u16,
    pub progression_gap_bps: u16,
    pub economy_inflation_bps: u16,
    pub guild_dominance_bps: u16,
}

impl DirectorPressureScores {
    pub fn from_snapshot(snapshot: &WorldTelemetrySnapshot) -> Result<Self, String> {
        snapshot.validate()?;
        let total_players = snapshot
            .maps
            .iter()
            .map(|map| u64::from(map.active_players))
            .sum::<u64>();
        let busiest = snapshot
            .maps
            .iter()
            .map(|map| u64::from(map.active_players))
            .max()
            .unwrap_or_default();
        let population_imbalance_bps = if total_players == 0 {
            0
        } else {
            bps(
                busiest.saturating_mul(snapshot.maps.len() as u64),
                total_players,
            )
            .saturating_sub(BASIS_POINTS)
        };

        let kills = snapshot
            .maps
            .iter()
            .map(|map| map.monster_kills)
            .sum::<u64>();
        let quests = snapshot
            .maps
            .iter()
            .map(|map| map.completed_quests)
            .sum::<u64>();
        let content_fatigue_bps = if kills == 0 {
            0
        } else {
            bps(kills.saturating_sub(quests.saturating_mul(20)), kills)
        };

        let weighted_level_sum = snapshot
            .maps
            .iter()
            .map(|map| u64::from(map.median_level) * u64::from(map.active_players))
            .sum::<u64>();
        let world_level = weighted_level_sum / total_players.max(1);
        let newcomer_count = snapshot
            .maps
            .iter()
            .map(|map| u64::from(map.new_player_count + map.returning_player_count))
            .sum::<u64>();
        let newcomer_level = snapshot
            .maps
            .iter()
            .filter(|map| map.new_player_count + map.returning_player_count > 0)
            .map(|map| u64::from(map.median_level))
            .min()
            .unwrap_or(world_level);
        let level_gap = world_level.saturating_sub(newcomer_level);
        let progression_gap_bps = bps(
            level_gap.saturating_mul(newcomer_count),
            world_level.max(1).saturating_mul(total_players.max(1)),
        );

        let net_gold = snapshot
            .economy
            .gold_created
            .saturating_sub(snapshot.economy.gold_destroyed);
        let monetary_inflation = bps(net_gold, snapshot.economy.gold_created.max(1));
        let price_inflation = u64::from(
            snapshot
                .economy
                .median_trade_price_index_bps
                .saturating_sub(BASIS_POINTS as u32),
        );
        let economy_inflation_bps = monetary_inflation.max(price_inflation);
        let guild_dominance_bps = u64::from(
            snapshot
                .guilds
                .largest_guild_population_bps
                .max(snapshot.guilds.largest_guild_boss_kill_share_bps),
        );

        Ok(Self {
            population_imbalance_bps: clamp_bps(population_imbalance_bps),
            content_fatigue_bps: clamp_bps(content_fatigue_bps),
            progression_gap_bps: clamp_bps(progression_gap_bps),
            economy_inflation_bps: clamp_bps(economy_inflation_bps),
            guild_dominance_bps: clamp_bps(guild_dominance_bps),
        })
    }

    pub fn score(&self, pressure: DirectorPressure) -> u16 {
        match pressure {
            DirectorPressure::PopulationImbalance => self.population_imbalance_bps,
            DirectorPressure::ContentFatigue => self.content_fatigue_bps,
            DirectorPressure::ProgressionGap => self.progression_gap_bps,
            DirectorPressure::EconomyInflation => self.economy_inflation_bps,
            DirectorPressure::GuildDominance => self.guild_dominance_bps,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DirectorAction {
    BroadcastRumor {
        message_key: String,
    },
    SpawnEncounter {
        zone_id: String,
        encounter_id: String,
        count: u16,
    },
    OpenCatchupQuest {
        quest_id: String,
        minimum_level: u16,
        maximum_level: u16,
    },
    ApplyRewardModifier {
        zone_id: String,
        modifier_bps: u16,
    },
    MutateBoss {
        zone_id: String,
        boss_id: String,
        variant_id: String,
    },
}

impl DirectorAction {
    fn validate(&self, allowed_zones: &BTreeSet<String>) -> Result<(), String> {
        match self {
            DirectorAction::BroadcastRumor { message_key } => {
                validate_component("director message key", message_key)
            }
            DirectorAction::SpawnEncounter {
                zone_id,
                encounter_id,
                count,
            } => {
                validate_allowed_zone(zone_id, allowed_zones)?;
                validate_component("director encounter id", encounter_id)?;
                if *count == 0 || *count > 500 {
                    return Err("director encounter count must be 1..=500".to_string());
                }
                Ok(())
            }
            DirectorAction::OpenCatchupQuest {
                quest_id,
                minimum_level,
                maximum_level,
            } => {
                validate_component("director quest id", quest_id)?;
                if minimum_level > maximum_level || *maximum_level > 255 {
                    return Err("director catch-up level range is invalid".to_string());
                }
                Ok(())
            }
            DirectorAction::ApplyRewardModifier {
                zone_id,
                modifier_bps,
            } => {
                validate_allowed_zone(zone_id, allowed_zones)?;
                if *modifier_bps < 10_000 || *modifier_bps > 15_000 {
                    return Err("director reward modifier must be 10000..=15000 bps".to_string());
                }
                Ok(())
            }
            DirectorAction::MutateBoss {
                zone_id,
                boss_id,
                variant_id,
            } => {
                validate_allowed_zone(zone_id, allowed_zones)?;
                validate_component("director boss id", boss_id)?;
                validate_component("director boss variant id", variant_id)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectorStage {
    pub stage_id: String,
    pub starts_after_ms: u64,
    pub actions: Vec<DirectorAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectorEventTemplate {
    pub template_id: String,
    pub display_name: String,
    pub primary_pressure: DirectorPressure,
    pub minimum_pressure_bps: u16,
    pub allowed_zones: BTreeSet<String>,
    pub maximum_duration_ms: u64,
    pub cooldown_ms: u64,
    pub maximum_reward_budget: u64,
    pub stages: Vec<DirectorStage>,
}

impl DirectorEventTemplate {
    pub fn validate(&self) -> Result<(), String> {
        validate_component("director template id", &self.template_id)?;
        validate_text("director template name", &self.display_name)?;
        if self.minimum_pressure_bps > BASIS_POINTS as u16 {
            return Err("director minimum pressure exceeds 10000 bps".to_string());
        }
        if self.allowed_zones.is_empty()
            || self.maximum_duration_ms == 0
            || self.cooldown_ms == 0
            || self.maximum_reward_budget == 0
            || self.stages.is_empty()
            || self.stages.len() > 16
        {
            return Err("director template has invalid empty or zero limits".to_string());
        }
        let mut stage_ids = BTreeSet::new();
        let mut last_offset = 0;
        for stage in &self.stages {
            validate_component("director stage id", &stage.stage_id)?;
            if !stage_ids.insert(stage.stage_id.as_str()) {
                return Err(format!("duplicate director stage {}", stage.stage_id));
            }
            if stage.starts_after_ms < last_offset
                || stage.starts_after_ms >= self.maximum_duration_ms
                || stage.actions.is_empty()
                || stage.actions.len() > 32
            {
                return Err(format!(
                    "director stage {} has invalid timing/actions",
                    stage.stage_id
                ));
            }
            last_offset = stage.starts_after_ms;
            for action in &stage.actions {
                action.validate(&self.allowed_zones)?;
            }
        }
        Ok(())
    }

    pub fn bichon_wooma() -> Self {
        let allowed_zones = ["map:0", "map:D022", "map:D023", "map:D024"]
            .into_iter()
            .map(str::to_string)
            .collect();
        Self {
            template_id: "mir2.bichon-wooma-awakening.v1".to_string(),
            display_name: "比奇—沃玛教主复苏".to_string(),
            primary_pressure: DirectorPressure::ContentFatigue,
            minimum_pressure_bps: 5_000,
            allowed_zones,
            maximum_duration_ms: 45 * 60 * 1_000,
            cooldown_ms: 6 * 60 * 60 * 1_000,
            maximum_reward_budget: 250_000,
            stages: vec![
                DirectorStage {
                    stage_id: "bichon-rumor".to_string(),
                    starts_after_ms: 0,
                    actions: vec![
                        DirectorAction::BroadcastRumor {
                            message_key: "world.wooma.rumor.bichon".to_string(),
                        },
                        DirectorAction::OpenCatchupQuest {
                            quest_id: "quest.wooma-scout.v1".to_string(),
                            minimum_level: 15,
                            maximum_level: 28,
                        },
                    ],
                },
                DirectorStage {
                    stage_id: "temple-incursion".to_string(),
                    starts_after_ms: 5 * 60 * 1_000,
                    actions: vec![
                        DirectorAction::SpawnEncounter {
                            zone_id: "map:D022".to_string(),
                            encounter_id: "encounter.wooma-vanguard.v1".to_string(),
                            count: 24,
                        },
                        DirectorAction::ApplyRewardModifier {
                            zone_id: "map:D022".to_string(),
                            modifier_bps: 11_500,
                        },
                    ],
                },
                DirectorStage {
                    stage_id: "wooma-taurus-awakens".to_string(),
                    starts_after_ms: 20 * 60 * 1_000,
                    actions: vec![
                        DirectorAction::MutateBoss {
                            zone_id: "map:D024".to_string(),
                            boss_id: "WoomaTaurus".to_string(),
                            variant_id: "director.awakened.v1".to_string(),
                        },
                        DirectorAction::BroadcastRumor {
                            message_key: "world.wooma.taurus.awakened".to_string(),
                        },
                    ],
                },
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DirectorProposalSource {
    RuleEngine { rule_id: String },
    Ai { provider: String, model: String },
    Operator { operator_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DirectorProposal {
    pub proposal_id: String,
    pub snapshot_id: String,
    pub template_id: String,
    pub source: DirectorProposalSource,
    pub target_zones: BTreeSet<String>,
    pub duration_ms: u64,
    pub reward_budget: u64,
    pub seed: u64,
    pub generation: u64,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectorTemplateSummary {
    pub template_id: String,
    pub primary_pressure: DirectorPressure,
    pub minimum_pressure_bps: u16,
    pub allowed_zones: BTreeSet<String>,
    pub maximum_duration_ms: u64,
    pub maximum_reward_budget: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiDirectorProposalRequest {
    pub schema: String,
    pub snapshot: WorldTelemetrySnapshot,
    pub pressure_scores: DirectorPressureScores,
    pub templates: Vec<DirectorTemplateSummary>,
    pub budget: DirectorBudgetPolicy,
    pub instruction: String,
}

/// Strict JSON boundary for an optional model provider. It accepts only a
/// template choice and bounded parameters; actions are always materialized
/// from the server-side allowlisted catalog after policy approval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiDirectorProposalAdapter {
    provider: String,
    model: String,
}

impl AiDirectorProposalAdapter {
    pub fn new(provider: impl Into<String>, model: impl Into<String>) -> Result<Self, String> {
        let provider = provider.into();
        let model = model.into();
        validate_component("AI director provider", &provider)?;
        validate_component("AI director model", &model)?;
        Ok(Self { provider, model })
    }

    pub fn decode(&self, response: &[u8]) -> Result<DirectorProposal, String> {
        if response.is_empty() || response.len() > MAX_AI_PROPOSAL_BYTES {
            return Err("AI director response must be 1..=16384 bytes".to_string());
        }
        let proposal: DirectorProposal = serde_json::from_slice(response).map_err(|error| {
            format!("AI director response is not strict proposal JSON: {error}")
        })?;
        match &proposal.source {
            DirectorProposalSource::Ai { provider, model }
                if provider == &self.provider && model == &self.model => {}
            _ => {
                return Err(
                    "AI director response source does not match the configured provider/model"
                        .to_string(),
                )
            }
        }
        validate_component("AI director proposal id", &proposal.proposal_id)?;
        validate_component("AI director snapshot id", &proposal.snapshot_id)?;
        validate_component("AI director template id", &proposal.template_id)?;
        validate_text("AI director rationale", &proposal.rationale)?;
        Ok(proposal)
    }
}

impl DirectorProposal {
    pub fn bichon_wooma_rule(
        snapshot: &WorldTelemetrySnapshot,
        scores: &DirectorPressureScores,
        now_ms: u64,
    ) -> Option<Self> {
        let template = DirectorEventTemplate::bichon_wooma();
        if scores.score(template.primary_pressure) < template.minimum_pressure_bps {
            return None;
        }
        let seed = stable_u64(&format!("{}:{now_ms}", snapshot.snapshot_id));
        Some(Self {
            proposal_id: format!("proposal:{}:{seed:016x}", snapshot.snapshot_id),
            snapshot_id: snapshot.snapshot_id.clone(),
            template_id: template.template_id,
            source: DirectorProposalSource::RuleEngine {
                rule_id: "mir2.content-fatigue.wooma.v1".to_string(),
            },
            target_zones: template.allowed_zones,
            duration_ms: 40 * 60 * 1_000,
            reward_budget: 150_000,
            seed,
            generation: 1,
            rationale: "沃玛区域内容疲劳超过阈值，以受限事件链重新组织现有内容".to_string(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectorBudgetPolicy {
    pub maximum_reward_budget_per_event: u64,
    pub maximum_reward_budget_per_day: u64,
    pub maximum_duration_ms: u64,
    pub maximum_target_zones: usize,
    pub maximum_concurrent_events: usize,
}

impl Default for DirectorBudgetPolicy {
    fn default() -> Self {
        Self {
            maximum_reward_budget_per_event: 250_000,
            maximum_reward_budget_per_day: 1_000_000,
            maximum_duration_ms: 60 * 60 * 1_000,
            maximum_target_zones: 8,
            maximum_concurrent_events: 2,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DirectorPolicyState {
    pub reward_budget_spent_today: u64,
    pub active_event_ids: BTreeSet<String>,
    pub template_last_finished_at_ms: BTreeMap<String, u64>,
}

#[derive(Debug, Clone)]
pub struct WorldDirectorPolicy {
    templates: BTreeMap<String, DirectorEventTemplate>,
    budget: DirectorBudgetPolicy,
}

impl WorldDirectorPolicy {
    pub fn new(
        templates: impl IntoIterator<Item = DirectorEventTemplate>,
        budget: DirectorBudgetPolicy,
    ) -> Result<Self, String> {
        let mut by_id = BTreeMap::new();
        for template in templates {
            template.validate()?;
            if by_id
                .insert(template.template_id.clone(), template)
                .is_some()
            {
                return Err("duplicate director template".to_string());
            }
        }
        if by_id.is_empty()
            || budget.maximum_reward_budget_per_event == 0
            || budget.maximum_reward_budget_per_day == 0
            || budget.maximum_duration_ms == 0
            || budget.maximum_target_zones == 0
            || budget.maximum_concurrent_events == 0
        {
            return Err("director policy must contain templates and positive limits".to_string());
        }
        Ok(Self {
            templates: by_id,
            budget,
        })
    }

    pub fn mir2_default() -> Self {
        Self::new(
            [DirectorEventTemplate::bichon_wooma()],
            DirectorBudgetPolicy::default(),
        )
        .expect("built-in Mir2 director policy must be valid")
    }

    pub fn ai_request(
        &self,
        snapshot: &WorldTelemetrySnapshot,
        scores: &DirectorPressureScores,
    ) -> Result<AiDirectorProposalRequest, String> {
        snapshot.validate()?;
        let templates = self
            .templates
            .values()
            .map(|template| DirectorTemplateSummary {
                template_id: template.template_id.clone(),
                primary_pressure: template.primary_pressure,
                minimum_pressure_bps: template.minimum_pressure_bps,
                allowed_zones: template.allowed_zones.clone(),
                maximum_duration_ms: template.maximum_duration_ms,
                maximum_reward_budget: template.maximum_reward_budget,
            })
            .collect();
        Ok(AiDirectorProposalRequest {
            schema: WORLD_DIRECTOR_SCHEMA.to_string(),
            snapshot: snapshot.clone(),
            pressure_scores: scores.clone(),
            templates,
            budget: self.budget.clone(),
            instruction: "只返回一个 JSON DirectorProposal；只能选择给定 templateId 和 zone，不能返回动作、数据库写入、资产铸造、封禁或自由脚本。"
                .to_string(),
        })
    }

    pub fn approve(
        &self,
        proposal: &DirectorProposal,
        snapshot: &WorldTelemetrySnapshot,
        scores: &DirectorPressureScores,
        state: &DirectorPolicyState,
        now_ms: u64,
    ) -> Result<ApprovedDirectorPlan, String> {
        snapshot.validate()?;
        validate_component("director proposal id", &proposal.proposal_id)?;
        validate_text("director rationale", &proposal.rationale)?;
        if proposal.snapshot_id != snapshot.snapshot_id {
            return Err("director proposal is bound to a different snapshot".to_string());
        }
        if proposal.generation == 0 || proposal.seed == 0 {
            return Err("director proposal generation and seed must be positive".to_string());
        }
        let template = self
            .templates
            .get(&proposal.template_id)
            .ok_or_else(|| "director proposal references a non-allowlisted template".to_string())?;
        if scores.score(template.primary_pressure) < template.minimum_pressure_bps {
            return Err("director proposal pressure threshold is not satisfied".to_string());
        }
        if proposal.target_zones.is_empty()
            || proposal.target_zones.len() > self.budget.maximum_target_zones
            || !proposal.target_zones.is_subset(&template.allowed_zones)
        {
            return Err("director proposal targets invalid or too many zones".to_string());
        }
        let observed_zones = snapshot
            .maps
            .iter()
            .map(|map| map.zone_id.as_str())
            .collect::<BTreeSet<_>>();
        if !proposal
            .target_zones
            .iter()
            .all(|zone| observed_zones.contains(zone.as_str()))
        {
            return Err("director proposal targets a zone absent from its snapshot".to_string());
        }
        if proposal.duration_ms == 0
            || proposal.duration_ms > template.maximum_duration_ms
            || proposal.duration_ms > self.budget.maximum_duration_ms
        {
            return Err("director proposal duration exceeds policy".to_string());
        }
        if proposal.reward_budget == 0
            || proposal.reward_budget > template.maximum_reward_budget
            || proposal.reward_budget > self.budget.maximum_reward_budget_per_event
            || state
                .reward_budget_spent_today
                .saturating_add(proposal.reward_budget)
                > self.budget.maximum_reward_budget_per_day
        {
            return Err("director proposal reward budget exceeds policy".to_string());
        }
        if state.active_event_ids.len() >= self.budget.maximum_concurrent_events {
            return Err("director concurrent event limit reached".to_string());
        }
        if let Some(last_finished) = state
            .template_last_finished_at_ms
            .get(&proposal.template_id)
        {
            if now_ms < last_finished.saturating_add(template.cooldown_ms) {
                return Err("director template is cooling down".to_string());
            }
        }
        Ok(ApprovedDirectorPlan {
            proposal: proposal.clone(),
            template: template.clone(),
            approved_at_ms: now_ms,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovedDirectorPlan {
    pub proposal: DirectorProposal,
    pub template: DirectorEventTemplate,
    pub approved_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectorCommandPayload {
    pub schema: String,
    pub command_id: String,
    pub proposal_id: String,
    pub snapshot_id: String,
    pub game_id: String,
    pub region_id: String,
    pub template_id: String,
    pub generation: u64,
    pub seed: u64,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
    pub duration_ms: u64,
    pub reward_budget: u64,
    pub target_zones: BTreeSet<String>,
    pub stages: Vec<DirectorStage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignedDirectorCommand {
    pub payload: DirectorCommandPayload,
    pub issuer_public_key: String,
    pub signature_algorithm: String,
    pub signature: String,
}

impl SignedDirectorCommand {
    pub fn issue(
        plan: &ApprovedDirectorPlan,
        snapshot: &WorldTelemetrySnapshot,
        issuer: &NodeSigningIdentity,
        validity_ms: u64,
    ) -> Result<Self, String> {
        if validity_ms == 0 {
            return Err("director command validity must be positive".to_string());
        }
        let mut payload = DirectorCommandPayload {
            schema: WORLD_DIRECTOR_SCHEMA.to_string(),
            command_id: String::new(),
            proposal_id: plan.proposal.proposal_id.clone(),
            snapshot_id: plan.proposal.snapshot_id.clone(),
            game_id: snapshot.game_id.clone(),
            region_id: snapshot.region_id.clone(),
            template_id: plan.proposal.template_id.clone(),
            generation: plan.proposal.generation,
            seed: plan.proposal.seed,
            issued_at_ms: plan.approved_at_ms,
            expires_at_ms: plan.approved_at_ms.saturating_add(validity_ms),
            duration_ms: plan.proposal.duration_ms,
            reward_budget: plan.proposal.reward_budget,
            target_zones: plan.proposal.target_zones.clone(),
            stages: plan.template.stages.clone(),
        };
        payload.command_id = payload.compute_id()?;
        let mut command = Self {
            payload,
            issuer_public_key: issuer.public_key().to_string(),
            signature_algorithm: "ed25519-zip215".to_string(),
            signature: String::new(),
        };
        command.signature = issuer.sign(&command.signing_bytes()?);
        Ok(command)
    }

    pub fn verify(&self, trusted_issuer: &str, now_ms: u64) -> Result<(), String> {
        if self.payload.schema != WORLD_DIRECTOR_SCHEMA {
            return Err("unsupported director command schema".to_string());
        }
        if self.issuer_public_key != trusted_issuer {
            return Err("director command issuer is not trusted".to_string());
        }
        if self.signature_algorithm != "ed25519-zip215" {
            return Err("director command must use Ed25519".to_string());
        }
        if self.payload.command_id != self.payload.compute_id()? {
            return Err("director command id mismatch".to_string());
        }
        if now_ms < self.payload.issued_at_ms || now_ms > self.payload.expires_at_ms {
            return Err("director command is not active or has expired".to_string());
        }
        if self.payload.target_zones.is_empty() || self.payload.stages.is_empty() {
            return Err("director command has no target zones or stages".to_string());
        }
        for stage in &self.payload.stages {
            for action in &stage.actions {
                action.validate(&self.payload.target_zones)?;
            }
        }
        verify_ed25519_signature(
            &self.issuer_public_key,
            &self.signing_bytes()?,
            &self.signature,
        )
    }

    pub fn control_envelope(&self) -> Result<ControlCommandEnvelope, String> {
        ControlCommandEnvelope::json(
            WORLD_DIRECTOR_NAMESPACE,
            self.payload.command_id.clone(),
            self,
        )
    }

    fn signing_bytes(&self) -> Result<Vec<u8>, String> {
        let mut unsigned = self.clone();
        unsigned.signature.clear();
        domain_json(WORLD_DIRECTOR_COMMAND_DOMAIN, &unsigned)
    }
}

impl DirectorCommandPayload {
    fn compute_id(&self) -> Result<String, String> {
        let mut unsigned = self.clone();
        unsigned.command_id.clear();
        let bytes = domain_json(WORLD_DIRECTOR_COMMAND_DOMAIN, &unsigned)?;
        Ok(hex_digest(Sha256::digest(bytes)))
    }
}

pub fn director_commands_from_finalized(
    finalized: &FinalizedControlBlock,
    trusted_issuer: &str,
    now_ms: u64,
) -> Result<Vec<SignedDirectorCommand>, String> {
    let mut commands = Vec::new();
    for envelope in &finalized.block.commands {
        if envelope.namespace != WORLD_DIRECTOR_NAMESPACE {
            continue;
        }
        let command: SignedDirectorCommand = serde_json::from_slice(&envelope.payload)
            .map_err(|error| format!("director command JSON decode failed: {error}"))?;
        command.verify(trusted_issuer, now_ms)?;
        if envelope.idempotency_key != command.payload.command_id {
            return Err("director control idempotency key mismatch".to_string());
        }
        commands.push(command);
    }
    Ok(commands)
}

/// Cryptographic proof carried across the Operator HTTP trust boundary.
/// `FinalizedControlBlock.signers` preserves Commonware ordering semantics;
/// this wrapper proves that every listed committee member actually approved
/// the exact block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinalizedDirectorSubmission {
    pub finalized: FinalizedControlBlock,
    pub validator_signatures: BTreeMap<String, String>,
}

impl FinalizedDirectorSubmission {
    pub fn issue(
        finalized: FinalizedControlBlock,
        validators: &[NodeSigningIdentity],
    ) -> Result<Self, String> {
        let signing_bytes = finalized_director_signing_bytes(&finalized)?;
        let mut validator_signatures = BTreeMap::new();
        for validator in validators {
            if finalized.signers.contains(validator.public_key()) {
                validator_signatures.insert(
                    validator.public_key().to_string(),
                    validator.sign(&signing_bytes),
                );
            }
        }
        let submission = Self {
            finalized,
            validator_signatures,
        };
        let committee = validators
            .iter()
            .map(|validator| validator.public_key().to_string())
            .collect::<BTreeSet<_>>();
        submission.verify(&committee)?;
        Ok(submission)
    }

    pub fn verify(&self, committee: &BTreeSet<String>) -> Result<(), String> {
        self.finalized.block.verify_digest()?;
        if !committee.contains(&self.finalized.block.proposer) {
            return Err("world director finality proposer is not in the committee".to_string());
        }
        let faults = committee.len().saturating_sub(1) / 3;
        let quorum = committee.len().saturating_sub(faults);
        if committee.is_empty() || self.finalized.signers.len() < quorum {
            return Err(format!(
                "world director finality needs {quorum} validator signatures, got {}",
                self.finalized.signers.len()
            ));
        }
        if !self
            .finalized
            .signers
            .iter()
            .all(|signer| committee.contains(signer))
        {
            return Err("world director finality contains an unknown signer".to_string());
        }
        if self.validator_signatures.keys().collect::<BTreeSet<_>>()
            != self.finalized.signers.iter().collect::<BTreeSet<_>>()
        {
            return Err(
                "world director finality signatures do not exactly match finalized signers"
                    .to_string(),
            );
        }
        let signing_bytes = finalized_director_signing_bytes(&self.finalized)?;
        for (validator, signature) in &self.validator_signatures {
            verify_ed25519_signature(validator, &signing_bytes, signature)
                .map_err(|error| format!("invalid finality signature from {validator}: {error}"))?;
        }
        Ok(())
    }
}

fn finalized_director_signing_bytes(finalized: &FinalizedControlBlock) -> Result<Vec<u8>, String> {
    domain_json(WORLD_DIRECTOR_FINALITY_DOMAIN, &finalized.block)
}

#[derive(Debug, Clone, Default)]
pub struct DirectorReplayGuard {
    applied: BTreeMap<String, DirectorExecutionReceipt>,
}

impl DirectorReplayGuard {
    pub fn receipt(&self, command_id: &str) -> Option<&DirectorExecutionReceipt> {
        self.applied.get(command_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppliedDirectorStage {
    pub stage_id: String,
    pub scheduled_at_ms: u64,
    pub action_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectorExecutionReceipt {
    pub schema: String,
    pub command_id: String,
    pub control_height: u64,
    pub zone_host_id: String,
    pub accepted: bool,
    pub applied_stages: Vec<AppliedDirectorStage>,
    pub state_commitment: String,
    pub executed_at_ms: u64,
    pub executor_public_key: String,
    pub signature_algorithm: String,
    pub signature: String,
}

impl DirectorExecutionReceipt {
    pub fn verify(&self, trusted_executor: &str) -> Result<(), String> {
        if self.schema != WORLD_DIRECTOR_SCHEMA
            || self.executor_public_key != trusted_executor
            || self.signature_algorithm != "ed25519-zip215"
            || self.control_height == 0
            || self.command_id.is_empty()
        {
            return Err("director receipt metadata is invalid".to_string());
        }
        verify_ed25519_signature(
            &self.executor_public_key,
            &self.signing_bytes()?,
            &self.signature,
        )
    }

    fn signing_bytes(&self) -> Result<Vec<u8>, String> {
        let mut unsigned = self.clone();
        unsigned.signature.clear();
        domain_json(WORLD_DIRECTOR_RECEIPT_DOMAIN, &unsigned)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectorSimulationAdvanceReceipt {
    pub command_ids: Vec<String>,
    pub applied_action_keys: Vec<String>,
    pub spawned_monsters: usize,
    pub broadcast_messages: usize,
    pub active_reward_modifiers_bps: BTreeMap<String, u16>,
    pub state_commitment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DirectorSimulationCheckpoint {
    version: u32,
    installed_commands: BTreeMap<String, SignedDirectorCommand>,
    applied_action_keys: BTreeSet<String>,
    active_reward_modifiers_bps: BTreeMap<String, u16>,
    state_commitment: String,
}

/// Bridges finalized director schedules into the real shared Zone runtime.
/// Future stages survive process restart because both the signed commands and
/// applied-action keys are checkpointed. Spawned monsters themselves are
/// persisted by the existing ZoneRuntime checkpoint.
#[derive(Debug, Clone)]
pub struct Mir2DirectorSimulationAdapter {
    trusted_director: String,
    installed_commands: BTreeMap<String, SignedDirectorCommand>,
    applied_action_keys: BTreeSet<String>,
    active_reward_modifiers_bps: BTreeMap<String, u16>,
}

impl Mir2DirectorSimulationAdapter {
    pub fn new(trusted_director: impl Into<String>) -> Result<Self, String> {
        let trusted_director = trusted_director.into();
        validate_component("trusted world director public key", &trusted_director)?;
        Ok(Self {
            trusted_director,
            installed_commands: BTreeMap::new(),
            applied_action_keys: BTreeSet::new(),
            active_reward_modifiers_bps: BTreeMap::new(),
        })
    }

    pub fn install(&mut self, command: SignedDirectorCommand, now_ms: u64) -> Result<bool, String> {
        command.verify(&self.trusted_director, now_ms)?;
        if let Some(existing) = self.installed_commands.get(&command.payload.command_id) {
            if existing != &command {
                return Err("conflicting director command with an installed id".to_string());
            }
            return Ok(false);
        }
        self.installed_commands
            .insert(command.payload.command_id.clone(), command);
        Ok(true)
    }

    pub fn advance(
        &mut self,
        now_ms: u64,
        factory: &SharedInProcessZoneRuntimeFactory,
    ) -> Result<DirectorSimulationAdvanceReceipt, String> {
        self.advance_with_zone_router(now_ms, factory, |zone_id| ZoneId::new(zone_id.to_string()))
    }

    pub fn advance_with_zone_router(
        &mut self,
        now_ms: u64,
        factory: &SharedInProcessZoneRuntimeFactory,
        runtime_zone_id: impl Fn(&str) -> ZoneId,
    ) -> Result<DirectorSimulationAdvanceReceipt, String> {
        let commands = self
            .installed_commands
            .values()
            .cloned()
            .collect::<Vec<_>>();
        let mut command_ids = Vec::new();
        let mut applied_action_keys = Vec::new();
        let mut spawned_monsters = 0;
        let mut broadcast_messages = 0;

        for command in commands {
            let event_ends_at = command
                .payload
                .issued_at_ms
                .saturating_add(command.payload.duration_ms);
            if now_ms < command.payload.issued_at_ms || now_ms > event_ends_at {
                continue;
            }
            command_ids.push(command.payload.command_id.clone());
            let reward_modifiers = director_reward_modifiers(&command);
            for (stage_index, stage) in command.payload.stages.iter().enumerate() {
                if now_ms
                    < command
                        .payload
                        .issued_at_ms
                        .saturating_add(stage.starts_after_ms)
                {
                    continue;
                }
                for (action_index, action) in stage.actions.iter().enumerate() {
                    let action_key = format!(
                        "{}:{}:{}",
                        command.payload.command_id, stage_index, action_index
                    );
                    if self.applied_action_keys.contains(&action_key) {
                        continue;
                    }
                    match action {
                        DirectorAction::BroadcastRumor { message_key } => {
                            for zone in &command.payload.target_zones {
                                factory.broadcast_world_event_message(
                                    &runtime_zone_id(zone),
                                    director_map_file_name(zone)?,
                                    message_key,
                                    now_ms,
                                )?;
                                broadcast_messages += 1;
                            }
                        }
                        DirectorAction::OpenCatchupQuest {
                            quest_id,
                            minimum_level,
                            maximum_level,
                        } => {
                            let message = format!(
                                "{quest_id} available for Lv{minimum_level}-{maximum_level}"
                            );
                            for zone in &command.payload.target_zones {
                                factory.broadcast_world_event_message(
                                    &runtime_zone_id(zone),
                                    director_map_file_name(zone)?,
                                    &message,
                                    now_ms,
                                )?;
                                broadcast_messages += 1;
                            }
                        }
                        DirectorAction::ApplyRewardModifier {
                            zone_id,
                            modifier_bps,
                        } => {
                            self.active_reward_modifiers_bps
                                .insert(zone_id.clone(), *modifier_bps);
                        }
                        DirectorAction::SpawnEncounter {
                            zone_id,
                            encounter_id,
                            count,
                        } => {
                            let monster_name = match encounter_id.as_str() {
                                "encounter.wooma-vanguard.v1" => "WoomaSoldier",
                                _ => {
                                    return Err(format!(
                                    "director encounter {encounter_id} has no simulation adapter"
                                ))
                                }
                            };
                            let modifier = reward_modifiers
                                .get(zone_id)
                                .copied()
                                .unwrap_or(BASIS_POINTS as u16);
                            let spawns = director_monster_spawns(
                                &command,
                                &action_key,
                                zone_id,
                                monster_name,
                                *count,
                                modifier,
                                false,
                            )?;
                            spawned_monsters += factory.apply_world_event_monsters(
                                &runtime_zone_id(zone_id),
                                director_map_file_name(zone_id)?,
                                &spawns,
                                now_ms,
                            )?;
                        }
                        DirectorAction::MutateBoss {
                            zone_id,
                            boss_id,
                            variant_id,
                        } => {
                            if boss_id != "WoomaTaurus" || variant_id != "director.awakened.v1" {
                                return Err(
                                    "director boss mutation has no simulation adapter".to_string()
                                );
                            }
                            let modifier = reward_modifiers
                                .get(zone_id)
                                .copied()
                                .unwrap_or(BASIS_POINTS as u16);
                            let spawns = director_monster_spawns(
                                &command,
                                &action_key,
                                zone_id,
                                boss_id,
                                1,
                                modifier,
                                true,
                            )?;
                            spawned_monsters += factory.apply_world_event_monsters(
                                &runtime_zone_id(zone_id),
                                director_map_file_name(zone_id)?,
                                &spawns,
                                now_ms,
                            )?;
                        }
                    }
                    self.applied_action_keys.insert(action_key.clone());
                    applied_action_keys.push(action_key);
                }
            }
        }

        Ok(DirectorSimulationAdvanceReceipt {
            command_ids,
            applied_action_keys,
            spawned_monsters,
            broadcast_messages,
            active_reward_modifiers_bps: self.active_reward_modifiers_bps.clone(),
            state_commitment: self.state_commitment()?,
        })
    }

    pub fn checkpoint_bytes(&self) -> Result<Vec<u8>, String> {
        let mut checkpoint = DirectorSimulationCheckpoint {
            version: DIRECTOR_SIMULATION_CHECKPOINT_VERSION,
            installed_commands: self.installed_commands.clone(),
            applied_action_keys: self.applied_action_keys.clone(),
            active_reward_modifiers_bps: self.active_reward_modifiers_bps.clone(),
            state_commitment: String::new(),
        };
        checkpoint.state_commitment = simulation_checkpoint_commitment(&checkpoint)?;
        serde_json::to_vec(&checkpoint)
            .map_err(|error| format!("director simulation checkpoint encode failed: {error}"))
    }

    pub fn restore(bytes: &[u8], trusted_director: impl Into<String>) -> Result<Self, String> {
        let checkpoint: DirectorSimulationCheckpoint = serde_json::from_slice(bytes)
            .map_err(|error| format!("director simulation checkpoint decode failed: {error}"))?;
        if checkpoint.version != DIRECTOR_SIMULATION_CHECKPOINT_VERSION
            || checkpoint.state_commitment != simulation_checkpoint_commitment(&checkpoint)?
        {
            return Err("director simulation checkpoint version/commitment mismatch".to_string());
        }
        let adapter = Self {
            trusted_director: trusted_director.into(),
            installed_commands: checkpoint.installed_commands,
            applied_action_keys: checkpoint.applied_action_keys,
            active_reward_modifiers_bps: checkpoint.active_reward_modifiers_bps,
        };
        validate_component(
            "trusted world director public key",
            &adapter.trusted_director,
        )?;
        for command in adapter.installed_commands.values() {
            command.verify(&adapter.trusted_director, command.payload.issued_at_ms)?;
        }
        Ok(adapter)
    }

    fn state_commitment(&self) -> Result<String, String> {
        simulation_checkpoint_commitment(&DirectorSimulationCheckpoint {
            version: DIRECTOR_SIMULATION_CHECKPOINT_VERSION,
            installed_commands: self.installed_commands.clone(),
            applied_action_keys: self.applied_action_keys.clone(),
            active_reward_modifiers_bps: self.active_reward_modifiers_bps.clone(),
            state_commitment: String::new(),
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldDirectorCheckpointTelemetry {
    pub configured: bool,
    pub file_bytes: u64,
    pub last_zone_factory_bytes: u64,
    pub write_attempts_total: u64,
    pub writes_total: u64,
    pub write_failures_total: u64,
    pub write_bytes_total: u64,
    pub write_duration_ns_total: u64,
    pub write_last_bytes: u64,
    pub write_last_duration_ns: u64,
    pub last_success_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldDirectorRuntimeStatus {
    pub enabled: bool,
    pub finalized_height: u64,
    pub finalized_digest: Option<String>,
    pub installed_command_count: usize,
    pub applied_action_count: usize,
    pub active_reward_modifiers_bps: BTreeMap<String, u16>,
    pub world_event_monsters_by_zone: BTreeMap<String, usize>,
    pub world_event_monsters: BTreeMap<String, Vec<ZoneNativeMonsterSnapshot>>,
    pub spawned_monsters_total: u64,
    pub broadcast_messages_total: u64,
    pub last_advance_at_ms: u64,
    pub last_advance: Option<DirectorSimulationAdvanceReceipt>,
    #[serde(default)]
    pub checkpoint: WorldDirectorCheckpointTelemetry,
    pub state_commitment: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinalizedDirectorInstallReceipt {
    pub accepted: bool,
    pub finalized_height: u64,
    pub finalized_digest: String,
    pub newly_installed_commands: usize,
    pub advance: DirectorSimulationAdvanceReceipt,
}

/// Serde adapter that encodes binary checkpoint blobs as base64 strings instead
/// of the default JSON number arrays. Nested `Vec<u8>` payloads otherwise
/// inflate a multi-megabyte world image by roughly 4-5x and make the 30-second
/// world-director checkpoint write tens of megabytes large.
mod base64_bytes {
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine as _;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
        let encoded = String::deserialize(deserializer)?;
        STANDARD
            .decode(encoded.as_bytes())
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorldDirectorRuntimeCheckpoint {
    version: u32,
    finalized: Vec<FinalizedControlBlock>,
    #[serde(with = "base64_bytes", default)]
    simulation_checkpoint: Vec<u8>,
    #[serde(with = "base64_bytes", default)]
    zone_factory_checkpoint: Vec<u8>,
    spawned_monsters_total: u64,
    broadcast_messages_total: u64,
    last_advance_at_ms: u64,
    last_checkpoint_at_ms: u64,
    last_advance: Option<DirectorSimulationAdvanceReceipt>,
    state_commitment: String,
}

struct PersistedWorldDirectorCheckpoint {
    file_bytes: u64,
    zone_factory_bytes: u64,
}

struct WorldDirectorRuntimeState {
    control_log: CommonwareControlLog,
    adapter: Mir2DirectorSimulationAdapter,
    spawned_monsters_total: u64,
    broadcast_messages_total: u64,
    last_advance_at_ms: u64,
    last_checkpoint_at_ms: u64,
    last_advance: Option<DirectorSimulationAdvanceReceipt>,
}

pub type SharedDirectorZoneRouter = Arc<dyn Fn(&str) -> ZoneId + Send + Sync>;

/// Production bridge between finalized Commonware control blocks and the live
/// Mir2 Zone runtime. A single mutex deliberately serializes finality import,
/// stage application, and checkpoint publication so two operator requests
/// cannot apply the same world event concurrently.
pub struct WorldDirectorRuntimeService {
    committee: BTreeSet<String>,
    trusted_director: String,
    factory: Arc<SharedInProcessZoneRuntimeFactory>,
    runtime_zone_id: SharedDirectorZoneRouter,
    checkpoint_path: Option<PathBuf>,
    state: Mutex<WorldDirectorRuntimeState>,
    checkpoint_write_attempts_total: AtomicU64,
    checkpoint_writes_total: AtomicU64,
    checkpoint_write_failures_total: AtomicU64,
    checkpoint_write_bytes_total: AtomicU64,
    checkpoint_write_duration_ns_total: AtomicU64,
    checkpoint_write_last_bytes: AtomicU64,
    checkpoint_write_last_zone_factory_bytes: AtomicU64,
    checkpoint_write_last_duration_ns: AtomicU64,
    checkpoint_last_success_at_ms: AtomicU64,
}

impl WorldDirectorRuntimeService {
    pub fn new(
        committee: impl IntoIterator<Item = String>,
        trusted_director: impl Into<String>,
        factory: Arc<SharedInProcessZoneRuntimeFactory>,
        checkpoint_path: Option<PathBuf>,
    ) -> Result<Self, String> {
        Self::new_with_zone_router(
            committee,
            trusted_director,
            factory,
            checkpoint_path,
            Arc::new(|zone_id| ZoneId::new(zone_id.to_string())),
        )
    }

    pub fn new_with_zone_router(
        committee: impl IntoIterator<Item = String>,
        trusted_director: impl Into<String>,
        factory: Arc<SharedInProcessZoneRuntimeFactory>,
        checkpoint_path: Option<PathBuf>,
        runtime_zone_id: SharedDirectorZoneRouter,
    ) -> Result<Self, String> {
        let committee = committee.into_iter().collect::<BTreeSet<_>>();
        let trusted_director = trusted_director.into();
        let mut state = WorldDirectorRuntimeState {
            control_log: CommonwareControlLog::new(committee.iter().cloned())?,
            adapter: Mir2DirectorSimulationAdapter::new(trusted_director.clone())?,
            spawned_monsters_total: 0,
            broadcast_messages_total: 0,
            last_advance_at_ms: 0,
            last_checkpoint_at_ms: 0,
            last_advance: None,
        };
        let mut restored_checkpoint_bytes = 0;
        if let Some(path) = checkpoint_path.as_deref() {
            if let Some((restored, file_bytes)) = restore_runtime_checkpoint(
                path,
                &committee.iter().cloned().collect::<Vec<_>>(),
                &trusted_director,
                factory.as_ref(),
            )? {
                state = restored;
                restored_checkpoint_bytes = file_bytes;
            }
        }
        let restored_checkpoint_at_ms = state.last_checkpoint_at_ms;
        Ok(Self {
            committee,
            trusted_director,
            factory,
            runtime_zone_id,
            checkpoint_path,
            state: Mutex::new(state),
            checkpoint_write_attempts_total: AtomicU64::new(0),
            checkpoint_writes_total: AtomicU64::new(0),
            checkpoint_write_failures_total: AtomicU64::new(0),
            checkpoint_write_bytes_total: AtomicU64::new(0),
            checkpoint_write_duration_ns_total: AtomicU64::new(0),
            checkpoint_write_last_bytes: AtomicU64::new(restored_checkpoint_bytes),
            checkpoint_write_last_zone_factory_bytes: AtomicU64::new(0),
            checkpoint_write_last_duration_ns: AtomicU64::new(0),
            checkpoint_last_success_at_ms: AtomicU64::new(restored_checkpoint_at_ms),
        })
    }

    pub fn install_submission(
        &self,
        submission: FinalizedDirectorSubmission,
        now_ms: u64,
    ) -> Result<FinalizedDirectorInstallReceipt, String> {
        submission.verify(&self.committee)?;
        self.install_finalized(submission.finalized, now_ms)
    }

    pub fn install_finalized(
        &self,
        finalized: FinalizedControlBlock,
        now_ms: u64,
    ) -> Result<FinalizedDirectorInstallReceipt, String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "world director runtime mutex poisoned".to_string())?;
        if let Some(existing) = state
            .control_log
            .finalized()
            .into_iter()
            .find(|existing| existing.block.height == finalized.block.height)
        {
            if existing != finalized {
                return Err(format!(
                    "conflicting finalized block at height {}",
                    finalized.block.height
                ));
            }
            let advance = advance_runtime_state(
                &mut state,
                now_ms,
                self.factory.as_ref(),
                self.runtime_zone_id.as_ref(),
            )?;
            state.last_checkpoint_at_ms = now_ms;
            self.persist_locked(&state)?;
            return Ok(FinalizedDirectorInstallReceipt {
                accepted: false,
                finalized_height: existing.block.height,
                finalized_digest: existing.block.digest,
                newly_installed_commands: 0,
                advance,
            });
        }

        let commands =
            director_commands_from_finalized(&finalized, &self.trusted_director, now_ms)?;
        state.control_log.import_finalized(finalized.clone())?;
        let mut newly_installed_commands = 0;
        for command in commands {
            newly_installed_commands += usize::from(state.adapter.install(command, now_ms)?);
        }
        let advance = advance_runtime_state(
            &mut state,
            now_ms,
            self.factory.as_ref(),
            self.runtime_zone_id.as_ref(),
        )?;
        state.last_checkpoint_at_ms = now_ms;
        self.persist_locked(&state)?;
        Ok(FinalizedDirectorInstallReceipt {
            accepted: true,
            finalized_height: finalized.block.height,
            finalized_digest: finalized.block.digest,
            newly_installed_commands,
            advance,
        })
    }

    pub fn advance(&self, now_ms: u64) -> Result<DirectorSimulationAdvanceReceipt, String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "world director runtime mutex poisoned".to_string())?;
        let receipt = advance_runtime_state(
            &mut state,
            now_ms,
            self.factory.as_ref(),
            self.runtime_zone_id.as_ref(),
        )?;
        if !receipt.applied_action_keys.is_empty()
            || now_ms
                >= state
                    .last_checkpoint_at_ms
                    .saturating_add(DIRECTOR_RUNTIME_CHECKPOINT_INTERVAL_MS)
        {
            state.last_checkpoint_at_ms = now_ms;
            self.persist_locked(&state)?;
        }
        Ok(receipt)
    }

    pub fn status(&self) -> Result<WorldDirectorRuntimeStatus, String> {
        let state = self
            .state
            .lock()
            .map_err(|_| "world director runtime mutex poisoned".to_string())?;
        let finalized = state.control_log.finalized();
        let latest = finalized.last();
        let mut world_event_monsters_by_zone = BTreeMap::new();
        let mut world_event_monsters = BTreeMap::new();
        for zone_id in state
            .adapter
            .installed_commands
            .values()
            .flat_map(|command| command.payload.target_zones.iter())
            .collect::<BTreeSet<_>>()
        {
            let snapshots = self
                .factory
                .world_event_monster_snapshots(
                    &(self.runtime_zone_id)(zone_id),
                    director_map_file_name(zone_id)?,
                )?
                .into_iter()
                .filter(|monster| monster.object_id & 0xf000_0000 == 0x7000_0000)
                .collect::<Vec<_>>();
            world_event_monsters_by_zone.insert(zone_id.clone(), snapshots.len());
            world_event_monsters.insert(zone_id.clone(), snapshots);
        }
        Ok(WorldDirectorRuntimeStatus {
            enabled: true,
            finalized_height: latest.map(|block| block.block.height).unwrap_or_default(),
            finalized_digest: latest.map(|block| block.block.digest.clone()),
            installed_command_count: state.adapter.installed_commands.len(),
            applied_action_count: state.adapter.applied_action_keys.len(),
            active_reward_modifiers_bps: state.adapter.active_reward_modifiers_bps.clone(),
            world_event_monsters_by_zone,
            world_event_monsters,
            spawned_monsters_total: state.spawned_monsters_total,
            broadcast_messages_total: state.broadcast_messages_total,
            last_advance_at_ms: state.last_advance_at_ms,
            last_advance: state.last_advance.clone(),
            checkpoint: WorldDirectorCheckpointTelemetry {
                configured: self.checkpoint_path.is_some(),
                file_bytes: self
                    .checkpoint_path
                    .as_deref()
                    .and_then(|path| fs::metadata(path).ok())
                    .map(|metadata| metadata.len())
                    .unwrap_or_default(),
                last_zone_factory_bytes: self
                    .checkpoint_write_last_zone_factory_bytes
                    .load(Ordering::Acquire),
                write_attempts_total: self.checkpoint_write_attempts_total.load(Ordering::Acquire),
                writes_total: self.checkpoint_writes_total.load(Ordering::Acquire),
                write_failures_total: self.checkpoint_write_failures_total.load(Ordering::Acquire),
                write_bytes_total: self.checkpoint_write_bytes_total.load(Ordering::Acquire),
                write_duration_ns_total: self
                    .checkpoint_write_duration_ns_total
                    .load(Ordering::Acquire),
                write_last_bytes: self.checkpoint_write_last_bytes.load(Ordering::Acquire),
                write_last_duration_ns: self
                    .checkpoint_write_last_duration_ns
                    .load(Ordering::Acquire),
                last_success_at_ms: self.checkpoint_last_success_at_ms.load(Ordering::Acquire),
            },
            state_commitment: state.adapter.state_commitment()?,
        })
    }

    fn persist_locked(&self, state: &WorldDirectorRuntimeState) -> Result<(), String> {
        let Some(path) = self.checkpoint_path.as_deref() else {
            return Ok(());
        };
        self.checkpoint_write_attempts_total
            .fetch_add(1, Ordering::Relaxed);
        let started = Instant::now();
        let result = persist_runtime_checkpoint(path, state, self.factory.as_ref());
        let duration_ns = started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        self.checkpoint_write_duration_ns_total
            .fetch_add(duration_ns, Ordering::Relaxed);
        self.checkpoint_write_last_duration_ns
            .store(duration_ns, Ordering::Release);
        match result {
            Ok(checkpoint) => {
                self.checkpoint_writes_total.fetch_add(1, Ordering::Relaxed);
                self.checkpoint_write_bytes_total
                    .fetch_add(checkpoint.file_bytes, Ordering::Relaxed);
                self.checkpoint_write_last_bytes
                    .store(checkpoint.file_bytes, Ordering::Release);
                self.checkpoint_write_last_zone_factory_bytes
                    .store(checkpoint.zone_factory_bytes, Ordering::Release);
                self.checkpoint_last_success_at_ms
                    .store(state.last_checkpoint_at_ms, Ordering::Release);
                Ok(())
            }
            Err(error) => {
                self.checkpoint_write_failures_total
                    .fetch_add(1, Ordering::Relaxed);
                Err(error)
            }
        }
    }
}

fn advance_runtime_state(
    state: &mut WorldDirectorRuntimeState,
    now_ms: u64,
    factory: &SharedInProcessZoneRuntimeFactory,
    runtime_zone_id: &(dyn Fn(&str) -> ZoneId + Send + Sync),
) -> Result<DirectorSimulationAdvanceReceipt, String> {
    let receipt = state
        .adapter
        .advance_with_zone_router(now_ms, factory, runtime_zone_id)?;
    state.spawned_monsters_total = state
        .spawned_monsters_total
        .saturating_add(receipt.spawned_monsters as u64);
    state.broadcast_messages_total = state
        .broadcast_messages_total
        .saturating_add(receipt.broadcast_messages as u64);
    state.last_advance_at_ms = now_ms;
    state.last_advance = Some(receipt.clone());
    Ok(receipt)
}

#[derive(serde::Deserialize)]
struct RuntimeCheckpointVersionProbe {
    version: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DirectorCheckpointFileIdentity {
    volume_id: u64,
    object_id: [u8; 16],
    length: u64,
    modified_high: u64,
    modified_low: u64,
}

fn validate_director_checkpoint_metadata(
    metadata: &fs::Metadata,
    phase: &str,
) -> Result<(), String> {
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "world director checkpoint is not a regular file {phase}"
        ));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;

        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(format!(
                "world director checkpoint is a Windows reparse point {phase}"
            ));
        }
    }
    if metadata.len() > MAX_DIRECTOR_RUNTIME_CHECKPOINT_BYTES {
        return Err(format!(
            "world director checkpoint is {} bytes {phase} (limit {MAX_DIRECTOR_RUNTIME_CHECKPOINT_BYTES})",
            metadata.len()
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn open_director_checkpoint_no_follow(path: &Path) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;

    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn open_director_checkpoint_no_follow(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;

    const O_NOFOLLOW: i32 = 0x0002_0000;
    OpenOptions::new()
        .read(true)
        .custom_flags(O_NOFOLLOW)
        .open(path)
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
fn open_director_checkpoint_no_follow(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;

    const O_NOFOLLOW: i32 = 0x0000_0100;
    OpenOptions::new()
        .read(true)
        .custom_flags(O_NOFOLLOW)
        .open(path)
}

#[cfg(not(any(unix, windows)))]
fn open_director_checkpoint_no_follow(path: &Path) -> io::Result<File> {
    OpenOptions::new().read(true).open(path)
}

#[cfg(unix)]
fn director_checkpoint_file_identity(file: &File) -> io::Result<DirectorCheckpointFileIdentity> {
    use std::os::unix::fs::MetadataExt;

    let metadata = file.metadata()?;
    let mut object_id = [0u8; 16];
    object_id[..8].copy_from_slice(&metadata.ino().to_ne_bytes());
    Ok(DirectorCheckpointFileIdentity {
        volume_id: metadata.dev(),
        object_id,
        length: metadata.len(),
        modified_high: metadata.mtime() as u64,
        modified_low: metadata.mtime_nsec() as u64,
    })
}

#[cfg(windows)]
fn director_checkpoint_file_identity(file: &File) -> io::Result<DirectorCheckpointFileIdentity> {
    use std::ffi::c_void;
    use std::mem::{size_of, MaybeUninit};
    use std::os::windows::io::AsRawHandle;

    const FILE_ID_INFO_CLASS: i32 = 18;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct FileTime {
        low: u32,
        high: u32,
    }

    #[repr(C)]
    struct ByHandleFileInformation {
        file_attributes: u32,
        creation_time: FileTime,
        last_access_time: FileTime,
        last_write_time: FileTime,
        volume_serial_number: u32,
        file_size_high: u32,
        file_size_low: u32,
        number_of_links: u32,
        file_index_high: u32,
        file_index_low: u32,
    }

    #[repr(C)]
    struct FileIdInfo {
        volume_serial_number: u64,
        file_id: [u8; 16],
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetFileInformationByHandle(
            file: *mut c_void,
            information: *mut ByHandleFileInformation,
        ) -> i32;
        fn GetFileInformationByHandleEx(
            file: *mut c_void,
            information_class: i32,
            information: *mut c_void,
            buffer_size: u32,
        ) -> i32;
    }

    let raw_handle = file.as_raw_handle();
    let mut information = MaybeUninit::<ByHandleFileInformation>::uninit();
    if unsafe { GetFileInformationByHandle(raw_handle, information.as_mut_ptr()) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let information = unsafe { information.assume_init() };
    let mut file_id = MaybeUninit::<FileIdInfo>::uninit();
    if unsafe {
        GetFileInformationByHandleEx(
            raw_handle,
            FILE_ID_INFO_CLASS,
            file_id.as_mut_ptr().cast::<c_void>(),
            size_of::<FileIdInfo>() as u32,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let file_id = unsafe { file_id.assume_init() };
    if file_id.file_id.iter().all(|byte| *byte == 0) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Windows returned an all-zero world director checkpoint file identifier",
        ));
    }
    Ok(DirectorCheckpointFileIdentity {
        volume_id: file_id.volume_serial_number,
        object_id: file_id.file_id,
        length: (u64::from(information.file_size_high) << 32)
            | u64::from(information.file_size_low),
        modified_high: u64::from(information.last_write_time.high),
        modified_low: u64::from(information.last_write_time.low),
    })
}

#[cfg(not(any(unix, windows)))]
fn director_checkpoint_file_identity(file: &File) -> io::Result<DirectorCheckpointFileIdentity> {
    let metadata = file.metadata()?;
    let modified = metadata
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    Ok(DirectorCheckpointFileIdentity {
        volume_id: 0,
        object_id: [0; 16],
        length: metadata.len(),
        modified_high: modified.as_secs(),
        modified_low: u64::from(modified.subsec_nanos()),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DirectorCheckpointReadPhase {
    AncestorsGuarded,
    FileOpened,
    BeforePathReopen,
}

fn read_runtime_checkpoint_bytes(path: &Path) -> Result<Option<(Vec<u8>, u64)>, String> {
    read_runtime_checkpoint_bytes_with_callback(path, |_| Ok(()))
}

fn read_runtime_checkpoint_bytes_with_callback<F>(
    path: &Path,
    mut hook: F,
) -> Result<Option<(Vec<u8>, u64)>, String>
where
    F: FnMut(DirectorCheckpointReadPhase) -> Result<(), String>,
{
    let path = normalize_director_checkpoint_path(path)?;
    match fs::symlink_metadata(&path) {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            validate_missing_director_checkpoint_path(&path)?;
            return Ok(None);
        }
        Err(error) => {
            return Err(format!(
                "failed to inspect world director checkpoint {}: {error}",
                path.display()
            ));
        }
    }

    let directory = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| {
            format!(
                "world director checkpoint has no parent directory: {}",
                path.display()
            )
        })?;
    let ancestors = DirectorCheckpointAncestorGuard::capture(directory)?;
    hook(DirectorCheckpointReadPhase::AncestorsGuarded)?;
    ancestors.verify("before checkpoint open")?;

    let before = fs::symlink_metadata(&path).map_err(|error| {
        format!(
            "failed to re-inspect world director checkpoint {} after guarding ancestors: {error}",
            path.display()
        )
    })?;
    validate_director_checkpoint_metadata(&before, "before read")?;

    let mut file = open_director_checkpoint_no_follow(&path).map_err(|error| {
        format!(
            "failed to open world director checkpoint {} without following links: {error}",
            path.display()
        )
    })?;
    hook(DirectorCheckpointReadPhase::FileOpened)?;
    ancestors.verify("after checkpoint open")?;
    let opened_metadata = file.metadata().map_err(|error| {
        format!(
            "failed to inspect opened world director checkpoint {}: {error}",
            path.display()
        )
    })?;
    validate_director_checkpoint_metadata(&opened_metadata, "after open")?;
    let identity_before = director_checkpoint_file_identity(&file).map_err(|error| {
        format!(
            "failed to identify opened world director checkpoint {}: {error}",
            path.display()
        )
    })?;
    let mut bytes = Vec::with_capacity(identity_before.length.min(1024 * 1024) as usize);
    Read::by_ref(&mut file)
        .take(MAX_DIRECTOR_RUNTIME_CHECKPOINT_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| {
            format!(
                "failed to read world director checkpoint {}: {error}",
                path.display()
            )
        })?;
    if bytes.len() as u64 > MAX_DIRECTOR_RUNTIME_CHECKPOINT_BYTES {
        return Err(format!(
            "world director checkpoint exceeded the {MAX_DIRECTOR_RUNTIME_CHECKPOINT_BYTES}-byte limit while reading"
        ));
    }
    let identity_after_handle = director_checkpoint_file_identity(&file).map_err(|error| {
        format!(
            "failed to re-identify world director checkpoint {} after read: {error}",
            path.display()
        )
    })?;
    if identity_before != identity_after_handle || bytes.len() as u64 != identity_before.length {
        return Err(
            "world director checkpoint changed while its stable handle was read".to_string(),
        );
    }

    hook(DirectorCheckpointReadPhase::BeforePathReopen)?;
    ancestors.verify("before checkpoint path re-open")?;
    let reopened = open_director_checkpoint_no_follow(&path).map_err(|error| {
        format!(
            "failed to reopen world director checkpoint {} for identity verification: {error}",
            path.display()
        )
    })?;
    let reopened_metadata = reopened.metadata().map_err(|error| {
        format!(
            "failed to inspect reopened world director checkpoint {}: {error}",
            path.display()
        )
    })?;
    validate_director_checkpoint_metadata(&reopened_metadata, "after read")?;
    let identity_after_path = director_checkpoint_file_identity(&reopened).map_err(|error| {
        format!(
            "failed to identify reopened world director checkpoint {}: {error}",
            path.display()
        )
    })?;
    let after = fs::symlink_metadata(&path).map_err(|error| {
        format!(
            "failed to inspect world director checkpoint {} after read: {error}",
            path.display()
        )
    })?;
    validate_director_checkpoint_metadata(&after, "after read")?;
    ancestors.verify("after checkpoint read")?;
    if identity_before != identity_after_path || after.len() != identity_before.length {
        return Err(
            "world director checkpoint path identity changed during validation".to_string(),
        );
    }
    Ok(Some((bytes, identity_before.length)))
}

fn restore_runtime_checkpoint(
    path: &Path,
    committee: &[String],
    trusted_director: &str,
    factory: &SharedInProcessZoneRuntimeFactory,
) -> Result<Option<(WorldDirectorRuntimeState, u64)>, String> {
    let Some((bytes, file_bytes)) = read_runtime_checkpoint_bytes(path)? else {
        return Ok(None);
    };
    // Peek only the version field before the full decode so an incompatible
    // checkpoint fails with an explicit version error instead of an unrelated
    // base64 decode error from the older multi-megabyte number-array format.
    let stored_version = serde_json::from_slice::<RuntimeCheckpointVersionProbe>(&bytes)
        .map(|probe| probe.version)
        .map_err(|error| format!("world director runtime checkpoint decode failed: {error}"))?;
    if stored_version != DIRECTOR_RUNTIME_CHECKPOINT_VERSION {
        return Err(format!(
            "world director checkpoint version {stored_version} is not the current format (expected {DIRECTOR_RUNTIME_CHECKPOINT_VERSION}); refusing to start with configured incompatible state"
        ));
    }
    let checkpoint: WorldDirectorRuntimeCheckpoint = serde_json::from_slice(&bytes)
        .map_err(|error| format!("world director runtime checkpoint decode failed: {error}"))?;
    if checkpoint.version != DIRECTOR_RUNTIME_CHECKPOINT_VERSION
        || checkpoint.state_commitment != runtime_checkpoint_commitment(&checkpoint)?
    {
        return Err("world director runtime checkpoint version/commitment mismatch".to_string());
    }
    let control_log = CommonwareControlLog::new(committee.iter().cloned())?;
    for finalized in &checkpoint.finalized {
        control_log.import_finalized(finalized.clone())?;
    }
    let adapter = Mir2DirectorSimulationAdapter::restore(
        &checkpoint.simulation_checkpoint,
        trusted_director,
    )?;
    factory.install_world_checkpoint_bytes_atomically(&checkpoint.zone_factory_checkpoint)?;
    Ok(Some((
        WorldDirectorRuntimeState {
            control_log,
            adapter,
            spawned_monsters_total: checkpoint.spawned_monsters_total,
            broadcast_messages_total: checkpoint.broadcast_messages_total,
            last_advance_at_ms: checkpoint.last_advance_at_ms,
            last_checkpoint_at_ms: checkpoint.last_checkpoint_at_ms,
            last_advance: checkpoint.last_advance,
        },
        file_bytes,
    )))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DirectorCheckpointPublishPhase {
    TempCreated,
    TempWritten,
    TempSynced,
    BeforeRename,
    Renamed,
    BeforeDirectorySync,
    DirectorySynced,
}

fn validate_director_checkpoint_directory(
    path: &Path,
    metadata: &fs::Metadata,
) -> Result<(), String> {
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "world director checkpoint directory contains a non-directory or symbolic-link component: {}",
            path.display()
        ));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;

        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(format!(
                "world director checkpoint directory contains a Windows reparse-point component: {}",
                path.display()
            ));
        }
    }
    Ok(())
}

#[cfg(windows)]
fn open_director_checkpoint_directory_no_follow(path: &Path) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;

    const FILE_READ_ATTRIBUTES: u32 = 0x0000_0080;
    const FILE_SHARE_READ: u32 = 0x0000_0001;
    const FILE_SHARE_WRITE: u32 = 0x0000_0002;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    OpenOptions::new()
        .access_mode(FILE_READ_ATTRIBUTES)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn open_director_checkpoint_directory_no_follow(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;

    const O_NOFOLLOW: i32 = 0x0002_0000;
    OpenOptions::new()
        .read(true)
        .custom_flags(O_NOFOLLOW)
        .open(path)
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
fn open_director_checkpoint_directory_no_follow(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;

    const O_NOFOLLOW: i32 = 0x0000_0100;
    OpenOptions::new()
        .read(true)
        .custom_flags(O_NOFOLLOW)
        .open(path)
}

#[cfg(not(any(unix, windows)))]
fn open_director_checkpoint_directory_no_follow(path: &Path) -> io::Result<File> {
    OpenOptions::new().read(true).open(path)
}

fn normalize_director_checkpoint_path(path: &Path) -> Result<PathBuf, String> {
    if path.as_os_str().is_empty() {
        return Err("world director checkpoint path must not be empty".to_string());
    }
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| format!("failed to resolve checkpoint path: {error}"))?
            .join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            std::path::Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            std::path::Component::RootDir => {
                normalized.push(Path::new(std::path::MAIN_SEPARATOR_STR));
            }
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !normalized.pop() {
                    return Err(format!(
                        "world director checkpoint path escapes its filesystem root: {}",
                        path.display()
                    ));
                }
            }
            std::path::Component::Normal(component) => normalized.push(component),
        }
    }
    if !normalized.is_absolute() {
        return Err(format!(
            "failed to resolve an absolute world director checkpoint path: {}",
            path.display()
        ));
    }
    Ok(normalized)
}

fn director_checkpoint_same_object(
    left: DirectorCheckpointFileIdentity,
    right: DirectorCheckpointFileIdentity,
) -> bool {
    left.volume_id == right.volume_id && left.object_id == right.object_id
}

struct DirectorCheckpointAncestorEntry {
    path: PathBuf,
    handle: File,
    identity: DirectorCheckpointFileIdentity,
}

struct DirectorCheckpointAncestorGuard {
    entries: Vec<DirectorCheckpointAncestorEntry>,
}

impl DirectorCheckpointAncestorGuard {
    fn capture(directory: &Path) -> Result<Self, String> {
        let directory = normalize_director_checkpoint_path(directory)?;
        let mut entries = Vec::new();
        for ancestor in directory.ancestors().collect::<Vec<_>>().into_iter().rev() {
            if ancestor.as_os_str().is_empty() {
                continue;
            }
            let handle =
                open_director_checkpoint_directory_no_follow(ancestor).map_err(|error| {
                    format!(
                        "failed to open world director checkpoint ancestor {} without following links: {error}",
                        ancestor.display()
                    )
                })?;
            let metadata = handle.metadata().map_err(|error| {
                format!(
                    "failed to inspect opened world director checkpoint ancestor {}: {error}",
                    ancestor.display()
                )
            })?;
            validate_director_checkpoint_directory(ancestor, &metadata)?;
            let identity = director_checkpoint_file_identity(&handle).map_err(|error| {
                format!(
                    "failed to identify world director checkpoint ancestor {}: {error}",
                    ancestor.display()
                )
            })?;
            entries.push(DirectorCheckpointAncestorEntry {
                path: ancestor.to_path_buf(),
                handle,
                identity,
            });
        }
        if entries.is_empty() {
            return Err(
                "world director checkpoint has no verifiable directory ancestor".to_string(),
            );
        }
        Ok(Self { entries })
    }

    fn verify(&self, phase: &str) -> Result<(), String> {
        for entry in &self.entries {
            let held_metadata = entry.handle.metadata().map_err(|error| {
                format!(
                    "failed to inspect held world director checkpoint ancestor {} {phase}: {error}",
                    entry.path.display()
                )
            })?;
            validate_director_checkpoint_directory(&entry.path, &held_metadata)?;
            let held_identity =
                director_checkpoint_file_identity(&entry.handle).map_err(|error| {
                    format!(
                        "failed to re-identify held world director checkpoint ancestor {} {phase}: {error}",
                        entry.path.display()
                    )
                })?;
            if !director_checkpoint_same_object(entry.identity, held_identity) {
                return Err(format!(
                    "world director checkpoint ancestor handle identity changed {phase}: {}",
                    entry.path.display()
                ));
            }

            let reopened =
                open_director_checkpoint_directory_no_follow(&entry.path).map_err(|error| {
                    format!(
                        "failed to reopen world director checkpoint ancestor {} {phase}: {error}",
                        entry.path.display()
                    )
                })?;
            let reopened_metadata = reopened.metadata().map_err(|error| {
                format!(
                    "failed to inspect reopened world director checkpoint ancestor {} {phase}: {error}",
                    entry.path.display()
                )
            })?;
            validate_director_checkpoint_directory(&entry.path, &reopened_metadata)?;
            let reopened_identity =
                director_checkpoint_file_identity(&reopened).map_err(|error| {
                    format!(
                        "failed to identify reopened world director checkpoint ancestor {} {phase}: {error}",
                        entry.path.display()
                    )
                })?;
            if !director_checkpoint_same_object(entry.identity, reopened_identity) {
                return Err(format!(
                    "world director checkpoint ancestor path identity changed {phase}: {}",
                    entry.path.display()
                ));
            }
        }
        Ok(())
    }
}

fn validate_missing_director_checkpoint_path(path: &Path) -> Result<(), String> {
    let directory = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| {
            format!(
                "world director checkpoint has no parent directory: {}",
                path.display()
            )
        })?;
    let mut entries = Vec::new();
    let mut missing_component_seen = false;
    for ancestor in directory.ancestors().collect::<Vec<_>>().into_iter().rev() {
        if ancestor.as_os_str().is_empty() || missing_component_seen {
            continue;
        }
        let metadata = match fs::symlink_metadata(ancestor) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                missing_component_seen = true;
                continue;
            }
            Err(error) => {
                return Err(format!(
                    "failed to inspect missing-checkpoint ancestor {}: {error}",
                    ancestor.display()
                ));
            }
        };
        validate_director_checkpoint_directory(ancestor, &metadata)?;
        let handle = open_director_checkpoint_directory_no_follow(ancestor).map_err(|error| {
            format!(
                "failed to open missing-checkpoint ancestor {} without following links: {error}",
                ancestor.display()
            )
        })?;
        let opened_metadata = handle.metadata().map_err(|error| {
            format!(
                "failed to inspect opened missing-checkpoint ancestor {}: {error}",
                ancestor.display()
            )
        })?;
        validate_director_checkpoint_directory(ancestor, &opened_metadata)?;
        let identity = director_checkpoint_file_identity(&handle).map_err(|error| {
            format!(
                "failed to identify missing-checkpoint ancestor {}: {error}",
                ancestor.display()
            )
        })?;
        entries.push(DirectorCheckpointAncestorEntry {
            path: ancestor.to_path_buf(),
            handle,
            identity,
        });
    }
    if entries.is_empty() {
        return Err(
            "world director missing checkpoint has no verifiable existing ancestor".to_string(),
        );
    }
    let ancestors = DirectorCheckpointAncestorGuard { entries };
    ancestors.verify("while validating a missing checkpoint")?;
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(format!(
            "world director checkpoint appeared while its missing path was validated: {}",
            path.display()
        )),
        Err(error) => Err(format!(
            "world director missing checkpoint path is not traversable {}: {error}",
            path.display()
        )),
    }
}

fn ensure_director_checkpoint_directory_inner(
    path: &Path,
) -> Result<DirectorCheckpointAncestorGuard, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            validate_director_checkpoint_directory(path, &metadata)?;
            let guard = DirectorCheckpointAncestorGuard::capture(path)?;
            guard.verify("while accepting an existing checkpoint directory")?;
            return Ok(guard);
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "failed to inspect world director checkpoint directory {}: {error}",
                path.display()
            ));
        }
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| {
            format!(
                "missing world director checkpoint directory has no verifiable parent: {}",
                path.display()
            )
        })?;
    let parent_guard = ensure_director_checkpoint_directory_inner(parent)?;
    parent_guard.verify("immediately before checkpoint directory creation")?;
    match fs::create_dir(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let metadata = fs::symlink_metadata(path).map_err(|inspect_error| {
                format!(
                    "failed to inspect raced world director checkpoint directory {}: {inspect_error}",
                    path.display()
                )
            })?;
            validate_director_checkpoint_directory(path, &metadata)?;
        }
        Err(error) => {
            return Err(format!(
                "failed to create world director checkpoint directory {}: {error}",
                path.display()
            ));
        }
    }
    parent_guard.verify("immediately after checkpoint directory creation")?;
    sync_director_checkpoint_directory(parent).map_err(|error| {
        format!(
            "failed to sync parent of new world director checkpoint directory {}: {error}",
            parent.display()
        )
    })?;
    parent_guard.verify("after syncing a newly created checkpoint directory")?;
    let guard = DirectorCheckpointAncestorGuard::capture(path)?;
    guard.verify("after capturing a newly created checkpoint directory")?;
    Ok(guard)
}

fn ensure_director_checkpoint_directory(
    path: &Path,
) -> Result<(PathBuf, DirectorCheckpointAncestorGuard), String> {
    let checkpoint_path = normalize_director_checkpoint_path(path)?;
    let directory = checkpoint_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| {
            format!(
                "world director checkpoint has no parent directory: {}",
                checkpoint_path.display()
            )
        })?;
    let ancestors = ensure_director_checkpoint_directory_inner(directory)?;
    ancestors.verify("after checkpoint directory creation")?;
    Ok((checkpoint_path, ancestors))
}

#[cfg(unix)]
fn sync_director_checkpoint_directory(path: &Path) -> io::Result<()> {
    OpenOptions::new().read(true).open(path)?.sync_all()
}

#[cfg(windows)]
fn sync_director_checkpoint_directory(path: &Path) -> io::Result<()> {
    use std::os::windows::fs::OpenOptionsExt;

    const GENERIC_READ: u32 = 0x8000_0000;
    const GENERIC_WRITE: u32 = 0x4000_0000;
    const FILE_SHARE_READ: u32 = 0x0000_0001;
    const FILE_SHARE_WRITE: u32 = 0x0000_0002;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    OpenOptions::new()
        .access_mode(GENERIC_READ | GENERIC_WRITE)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?
        .sync_all()
}

#[cfg(not(any(unix, windows)))]
fn sync_director_checkpoint_directory(_path: &Path) -> io::Result<()> {
    Ok(())
}

struct CreatedDirectorCheckpointTemp {
    file: File,
    created_identity: DirectorCheckpointFileIdentity,
}

#[cfg(windows)]
fn create_director_checkpoint_temp(path: &Path) -> io::Result<CreatedDirectorCheckpointTemp> {
    use std::os::windows::fs::OpenOptionsExt;

    const DELETE: u32 = 0x0001_0000;
    const GENERIC_READ: u32 = 0x8000_0000;
    const GENERIC_WRITE: u32 = 0x4000_0000;
    const FILE_SHARE_READ: u32 = 0x0000_0001;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    let file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .access_mode(GENERIC_READ | GENERIC_WRITE | DELETE)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    let created_identity = director_checkpoint_file_identity(&file)?;
    Ok(CreatedDirectorCheckpointTemp {
        file,
        created_identity,
    })
}

#[cfg(unix)]
fn create_director_checkpoint_temp(path: &Path) -> io::Result<CreatedDirectorCheckpointTemp> {
    use std::os::unix::fs::OpenOptionsExt;

    let file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    let created_identity = director_checkpoint_file_identity(&file)?;
    Ok(CreatedDirectorCheckpointTemp {
        file,
        created_identity,
    })
}

#[cfg(not(any(unix, windows)))]
fn create_director_checkpoint_temp(path: &Path) -> io::Result<CreatedDirectorCheckpointTemp> {
    let file = OpenOptions::new().create_new(true).write(true).open(path)?;
    let created_identity = director_checkpoint_file_identity(&file)?;
    Ok(CreatedDirectorCheckpointTemp {
        file,
        created_identity,
    })
}

#[cfg(windows)]
fn windows_extended_path(path: &Path) -> io::Result<Vec<u16>> {
    use std::os::windows::ffi::OsStrExt;

    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let wide = absolute.as_os_str().encode_wide().collect::<Vec<_>>();
    if wide.starts_with(&[b'\\' as u16, b'\\' as u16, b'?' as u16, b'\\' as u16]) {
        Ok(wide)
    } else if wide.starts_with(&[b'\\' as u16, b'\\' as u16]) {
        let mut value = "\\\\?\\UNC\\".encode_utf16().collect::<Vec<_>>();
        value.extend_from_slice(&wide[2..]);
        Ok(value)
    } else {
        let mut value = "\\\\?\\".encode_utf16().collect::<Vec<_>>();
        value.extend_from_slice(&wide);
        Ok(value)
    }
}

#[cfg(windows)]
fn durable_replace_director_checkpoint(
    file: &File,
    _from: &Path,
    to: &Path,
    _ancestors: &DirectorCheckpointAncestorGuard,
) -> io::Result<()> {
    use std::ffi::c_void;
    use std::mem::{offset_of, size_of};
    use std::os::windows::io::AsRawHandle;
    use std::ptr;

    const FILE_RENAME_INFO_CLASS: i32 = 3;
    const FILE_RENAME_REPLACE_IF_EXISTS: u32 = 0x0000_0001;

    #[repr(C)]
    struct FileRenameInfo {
        flags: u32,
        root_directory: *mut c_void,
        file_name_length: u32,
        file_name: [u16; 1],
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn SetFileInformationByHandle(
            file: *mut c_void,
            information_class: i32,
            information: *const c_void,
            buffer_size: u32,
        ) -> i32;
    }

    let file_name = windows_extended_path(to)?;
    let file_name_bytes = file_name
        .len()
        .checked_mul(size_of::<u16>())
        .and_then(|length| u32::try_from(length).ok())
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "checkpoint path is too long")
        })?;
    let file_name_offset = offset_of!(FileRenameInfo, file_name);
    let buffer_size = file_name_offset
        .checked_add(file_name_bytes as usize)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "rename buffer overflow"))?;
    let word_size = size_of::<usize>();
    let mut storage = vec![0usize; (buffer_size + word_size - 1) / word_size];
    let information = storage.as_mut_ptr().cast::<FileRenameInfo>();
    unsafe {
        (*information).flags = FILE_RENAME_REPLACE_IF_EXISTS;
        (*information).root_directory = ptr::null_mut();
        (*information).file_name_length = file_name_bytes;
        ptr::copy_nonoverlapping(
            file_name.as_ptr(),
            (*information).file_name.as_mut_ptr(),
            file_name.len(),
        );
    }
    let renamed = unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle(),
            FILE_RENAME_INFO_CLASS,
            information.cast::<c_void>(),
            buffer_size as u32,
        )
    };
    if renamed == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(unix)]
fn durable_replace_director_checkpoint(
    _file: &File,
    from: &Path,
    to: &Path,
    ancestors: &DirectorCheckpointAncestorGuard,
) -> io::Result<()> {
    use std::ffi::{c_char, CString};
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::io::AsRawFd;

    extern "C" {
        fn renameat(
            old_directory: i32,
            old_path: *const c_char,
            new_directory: i32,
            new_path: *const c_char,
        ) -> i32;
    }

    if from.parent() != to.parent() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "checkpoint rename must remain within one guarded directory",
        ));
    }
    let directory = ancestors.entries.last().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "checkpoint ancestor guard is empty",
        )
    })?;
    let source = CString::new(
        from.file_name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "temp path has no name"))?
            .as_bytes(),
    )
    .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "temp name contains NUL"))?;
    let destination = CString::new(
        to.file_name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no name"))?
            .as_bytes(),
    )
    .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path name contains NUL"))?;
    let renamed = unsafe {
        renameat(
            directory.handle.as_raw_fd(),
            source.as_ptr(),
            directory.handle.as_raw_fd(),
            destination.as_ptr(),
        )
    };
    if renamed == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(not(any(unix, windows)))]
fn durable_replace_director_checkpoint(
    _file: &File,
    _from: &Path,
    _to: &Path,
    _ancestors: &DirectorCheckpointAncestorGuard,
) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "this platform has no controlled checkpoint rename implementation",
    ))
}

#[cfg(windows)]
fn delete_director_checkpoint_temp_by_handle(file: &File) -> io::Result<()> {
    use std::ffi::c_void;
    use std::mem::size_of;
    use std::os::windows::io::AsRawHandle;

    const FILE_DISPOSITION_INFO_CLASS: i32 = 4;

    #[repr(C)]
    struct FileDispositionInfo {
        // Windows BOOLEAN is an unsigned byte, not Win32 BOOL.
        delete_file: u8,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn SetFileInformationByHandle(
            file: *mut c_void,
            information_class: i32,
            information: *const c_void,
            buffer_size: u32,
        ) -> i32;
    }

    let information = FileDispositionInfo { delete_file: 1 };
    let deleted = unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle(),
            FILE_DISPOSITION_INFO_CLASS,
            (&information as *const FileDispositionInfo).cast::<c_void>(),
            size_of::<FileDispositionInfo>() as u32,
        )
    };
    if deleted == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn delete_director_checkpoint_temp_by_handle(_file: &File) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "safe handle-bound temporary checkpoint deletion is unavailable; leaving the file in place",
    ))
}

fn verify_director_checkpoint_path_identity(
    path: &Path,
    expected: DirectorCheckpointFileIdentity,
) -> Result<(), String> {
    let file = open_director_checkpoint_no_follow(path).map_err(|error| {
        format!(
            "failed to reopen world director checkpoint {} for publication verification: {error}",
            path.display()
        )
    })?;
    let metadata = file.metadata().map_err(|error| {
        format!(
            "failed to inspect published world director checkpoint {}: {error}",
            path.display()
        )
    })?;
    validate_director_checkpoint_metadata(&metadata, "after publication")?;
    let actual = director_checkpoint_file_identity(&file).map_err(|error| {
        format!(
            "failed to identify published world director checkpoint {}: {error}",
            path.display()
        )
    })?;
    if actual != expected {
        return Err(format!(
            "published world director checkpoint {} does not identify the synced temporary file",
            path.display()
        ));
    }
    Ok(())
}

fn safely_remove_director_checkpoint_temp(
    path: &Path,
    ancestors: &DirectorCheckpointAncestorGuard,
    created: CreatedDirectorCheckpointTemp,
) -> Result<(), String> {
    ancestors.verify("before temporary checkpoint cleanup")?;
    let handle_identity = director_checkpoint_file_identity(&created.file).map_err(|error| {
        format!(
            "failed to re-identify created temporary checkpoint {} for cleanup: {error}",
            path.display()
        )
    })?;
    if !director_checkpoint_same_object(created.created_identity, handle_identity) {
        return Err(format!(
            "created temporary checkpoint handle identity changed before cleanup: {}",
            path.display()
        ));
    }

    #[cfg(not(windows))]
    {
        match open_director_checkpoint_no_follow(path) {
            Ok(current_path_file) => {
                let current_path_identity = director_checkpoint_file_identity(&current_path_file)
                    .map_err(|error| {
                    format!(
                        "failed to identify current temporary checkpoint path {}: {error}",
                        path.display()
                    )
                })?;
                if !director_checkpoint_same_object(created.created_identity, current_path_identity)
                {
                    return Err(format!(
                        "temporary checkpoint path was replaced; refusing path-based cleanup: {}",
                        path.display()
                    ));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(format!("failed to inspect temp path for cleanup: {error}")),
        }
    }

    ancestors.verify("immediately before temporary checkpoint handle cleanup")?;
    delete_director_checkpoint_temp_by_handle(&created.file).map_err(|error| {
        format!(
            "failed to delete created temporary checkpoint by handle {}: {error}",
            path.display()
        )
    })?;
    drop(created.file);
    ancestors.verify("after temporary checkpoint handle cleanup")?;
    Ok(())
}

fn persist_runtime_checkpoint(
    path: &Path,
    state: &WorldDirectorRuntimeState,
    factory: &SharedInProcessZoneRuntimeFactory,
) -> Result<PersistedWorldDirectorCheckpoint, String> {
    persist_runtime_checkpoint_with_callback(path, state, factory, |_| Ok(()))
}

fn persist_runtime_checkpoint_with_callback<F>(
    path: &Path,
    state: &WorldDirectorRuntimeState,
    factory: &SharedInProcessZoneRuntimeFactory,
    mut hook: F,
) -> Result<PersistedWorldDirectorCheckpoint, String>
where
    F: FnMut(DirectorCheckpointPublishPhase) -> Result<(), String>,
{
    let (path, ancestors) = ensure_director_checkpoint_directory(path)?;
    let directory = path
        .parent()
        .expect("normalized checkpoint path must have a parent directory")
        .to_path_buf();
    ancestors.verify("before inspecting an existing checkpoint")?;
    match fs::symlink_metadata(&path) {
        Ok(metadata) => validate_director_checkpoint_metadata(&metadata, "before replacement")?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "failed to inspect existing world director checkpoint {}: {error}",
                path.display()
            ));
        }
    }
    let zone_factory_checkpoint = factory.world_checkpoint_bytes()?;
    let zone_factory_bytes = zone_factory_checkpoint.len() as u64;
    let mut checkpoint = WorldDirectorRuntimeCheckpoint {
        version: DIRECTOR_RUNTIME_CHECKPOINT_VERSION,
        finalized: state.control_log.finalized(),
        simulation_checkpoint: state.adapter.checkpoint_bytes()?,
        zone_factory_checkpoint,
        spawned_monsters_total: state.spawned_monsters_total,
        broadcast_messages_total: state.broadcast_messages_total,
        last_advance_at_ms: state.last_advance_at_ms,
        last_checkpoint_at_ms: state.last_checkpoint_at_ms,
        last_advance: state.last_advance.clone(),
        state_commitment: String::new(),
    };
    checkpoint.state_commitment = runtime_checkpoint_commitment(&checkpoint)?;
    let mut bytes = serde_json::to_vec(&checkpoint)
        .map_err(|error| format!("world director runtime checkpoint encode failed: {error}"))?;
    bytes.push(b'\n');
    if bytes.len() as u64 > MAX_DIRECTOR_RUNTIME_CHECKPOINT_BYTES {
        return Err(format!(
            "world director runtime checkpoint is {} bytes (limit {MAX_DIRECTOR_RUNTIME_CHECKPOINT_BYTES})",
            bytes.len()
        ));
    }

    let mut temporary_name = path
        .file_name()
        .ok_or_else(|| "world director checkpoint path has no file name".to_string())?
        .to_os_string();
    temporary_name.push(format!(
        ".{}.{}.tmp",
        std::process::id(),
        DIRECTOR_CHECKPOINT_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let temporary = directory.join(temporary_name);
    let mut published = false;
    let mut created_temp = None;
    let publication = (|| -> Result<DirectorCheckpointFileIdentity, String> {
        ancestors.verify("before temporary checkpoint creation")?;
        let created = create_director_checkpoint_temp(&temporary).map_err(|error| {
            format!(
                "failed to create world director checkpoint temp {}: {error}",
                temporary.display()
            )
        })?;
        created_temp = Some(created);
        let created = created_temp
            .as_mut()
            .expect("successful temp creation must retain its handle");
        let created_identity = created.created_identity;
        let file = &mut created.file;
        let created_metadata = file.metadata().map_err(|error| {
            format!(
                "failed to inspect world director checkpoint temp {}: {error}",
                temporary.display()
            )
        })?;
        validate_director_checkpoint_metadata(&created_metadata, "after temp create")?;
        hook(DirectorCheckpointPublishPhase::TempCreated)?;
        ancestors.verify("after temporary checkpoint creation")?;
        file.write_all(&bytes).map_err(|error| {
            format!(
                "failed to write world director checkpoint temp {}: {error}",
                temporary.display()
            )
        })?;
        hook(DirectorCheckpointPublishPhase::TempWritten)?;
        file.flush().map_err(|error| {
            format!(
                "failed to flush world director checkpoint temp {}: {error}",
                temporary.display()
            )
        })?;
        file.sync_all().map_err(|error| {
            format!(
                "failed to sync world director checkpoint temp {}: {error}",
                temporary.display()
            )
        })?;
        let written_identity = director_checkpoint_file_identity(&file).map_err(|error| {
            format!(
                "failed to identify synced world director checkpoint temp {}: {error}",
                temporary.display()
            )
        })?;
        if !director_checkpoint_same_object(created_identity, written_identity) {
            return Err(
                "synced temporary checkpoint no longer identifies the create_new file".to_string(),
            );
        }
        if written_identity.length != bytes.len() as u64 {
            return Err(
                "synced world director checkpoint temp has an unexpected length".to_string(),
            );
        }
        hook(DirectorCheckpointPublishPhase::TempSynced)?;
        verify_director_checkpoint_path_identity(&temporary, written_identity)?;
        ancestors.verify("after temporary checkpoint sync")?;
        hook(DirectorCheckpointPublishPhase::BeforeRename)?;
        ancestors.verify("immediately before checkpoint rename")?;
        verify_director_checkpoint_path_identity(&temporary, written_identity)?;
        durable_replace_director_checkpoint(file, &temporary, &path, &ancestors).map_err(
            |error| {
                format!(
                    "failed to atomically publish world director checkpoint {}: {error}",
                    path.display()
                )
            },
        )?;
        published = true;
        file.sync_all().map_err(|error| {
            format!("renamed checkpoint handle could not be synced after publication: {error}")
        })?;
        hook(DirectorCheckpointPublishPhase::Renamed)?;
        ancestors.verify("after checkpoint rename")?;
        verify_director_checkpoint_path_identity(&path, written_identity)?;
        hook(DirectorCheckpointPublishPhase::BeforeDirectorySync)?;
        ancestors.verify("before checkpoint directory sync")?;
        sync_director_checkpoint_directory(&directory).map_err(|error| {
            format!(
                "failed to sync world director checkpoint directory {}: {error}",
                directory.display()
            )
        })?;
        hook(DirectorCheckpointPublishPhase::DirectorySynced)?;
        ancestors.verify("after checkpoint directory sync")?;
        verify_director_checkpoint_path_identity(&path, written_identity)?;
        Ok(written_identity)
    })();
    if let Err(error) = publication {
        if published {
            return Err(format!(
                "world director checkpoint was published, but durability/verification is uncertain: {error}"
            ));
        }
        if let Some(created) = created_temp.take() {
            if let Err(cleanup_error) =
                safely_remove_director_checkpoint_temp(&temporary, &ancestors, created)
            {
                eprintln!(
                    "world director checkpoint temp cleanup failed closed for {}: {cleanup_error}",
                    temporary.display()
                );
            }
        }
        return Err(error);
    }
    Ok(PersistedWorldDirectorCheckpoint {
        file_bytes: bytes.len() as u64,
        zone_factory_bytes,
    })
}

fn runtime_checkpoint_commitment(
    checkpoint: &WorldDirectorRuntimeCheckpoint,
) -> Result<String, String> {
    let mut unsigned = checkpoint.clone();
    unsigned.state_commitment.clear();
    Ok(hex_digest(Sha256::digest(domain_json(
        DIRECTOR_RUNTIME_CHECKPOINT_DOMAIN,
        &unsigned,
    )?)))
}

fn director_reward_modifiers(command: &SignedDirectorCommand) -> BTreeMap<String, u16> {
    command
        .payload
        .stages
        .iter()
        .flat_map(|stage| stage.actions.iter())
        .filter_map(|action| match action {
            DirectorAction::ApplyRewardModifier {
                zone_id,
                modifier_bps,
            } => Some((zone_id.clone(), *modifier_bps)),
            _ => None,
        })
        .collect()
}

fn director_monster_spawns(
    command: &SignedDirectorCommand,
    action_key: &str,
    zone_id: &str,
    monster_name: &str,
    count: u16,
    reward_modifier_bps: u16,
    boss_mutation: bool,
) -> Result<Vec<ZoneMonsterSpawn>, String> {
    let template = crystal_monster_by_name(monster_name)
        .ok_or_else(|| format!("Crystal monster template {monster_name} is unavailable"))?;
    let map_file_name = director_map_file_name(zone_id)?;
    let respawn_map = crystal_map_respawns_by_file_name(&format!("{map_file_name}.map"))
        .or_else(|| crystal_map_respawns_by_file_name(map_file_name))
        .ok_or_else(|| format!("Crystal respawn map {map_file_name} is unavailable"))?;
    let matching_respawns = respawn_map
        .respawns
        .iter()
        .filter(|respawn| respawn.monster_name.eq_ignore_ascii_case(monster_name))
        .collect::<Vec<_>>();
    if matching_respawns.is_empty() {
        return Err(format!(
            "Crystal map {map_file_name} has no {monster_name} spawn anchor"
        ));
    }
    let action_seed = stable_u64(&format!("{}:{action_key}", command.payload.seed));
    let occupied_base_positions = respawn_map
        .respawns
        .iter()
        .flat_map(|respawn| crystal_world_respawn_spawns(map_file_name, respawn))
        .map(|(_, position, _)| (position.x, position.y))
        .collect::<BTreeSet<_>>();
    let candidate_count = count.saturating_mul(32).max(256);
    let mut placements = matching_respawns
        .into_iter()
        .enumerate()
        .flat_map(|(respawn_index, respawn)| {
            let mut event_respawn = (*respawn).clone();
            event_respawn.count = candidate_count;
            event_respawn.spread = event_respawn.spread.max(24);
            event_respawn.respawn_index = i32::try_from(
                stable_u64(&format!("{action_seed}:{respawn_index}:placement")) % i32::MAX as u64,
            )
            .unwrap_or(i32::MAX - 1);
            crystal_world_respawn_spawns(map_file_name, &event_respawn)
                .into_iter()
                .map(|(_, position, direction)| (position, direction))
        })
        .filter(|(position, _)| !occupied_base_positions.contains(&(position.x, position.y)))
        .collect::<Vec<_>>();
    placements.sort_by_key(|(position, _)| {
        (
            stable_u64(&format!("{action_seed}:{}:{}", position.x, position.y)),
            position.x,
            position.y,
        )
    });
    let mut used_positions = BTreeSet::new();
    placements.retain(|(position, _)| used_positions.insert((position.x, position.y)));
    if placements.len() < usize::from(count) {
        return Err(format!(
            "Crystal map {map_file_name} has only {} unique walkable {monster_name} placements; {count} required",
            placements.len()
        ));
    }
    let hp_multiplier = if boss_mutation {
        15_000_u64
    } else {
        BASIS_POINTS
    };
    Ok((0..count)
        .map(|index| {
            let (position, direction) = &placements[usize::from(index)];
            let object_id = 0x7000_0000_u32
                | (stable_u64(&format!("{action_key}:{index}")) as u32 & 0x0fff_ffff);
            let max_hp = (u64::try_from(template.hp.max(1)).unwrap_or(1) * hp_multiplier
                / BASIS_POINTS)
                .min(i32::MAX as u64) as i32;
            ZoneMonsterSpawn {
                object_id,
                name: template.name.clone(),
                name_colour_argb: if boss_mutation { -65_281 } else { -1 },
                image: template.image,
                ai: template.ai,
                disposition: Some(mir2_simulation::WorldEntityDisposition::Hostile),
                level: template.level,
                max_hp,
                hp: max_hp,
                experience: (u64::from(template.experience) * u64::from(reward_modifier_bps)
                    / BASIS_POINTS)
                    .min(u64::from(u32::MAX)) as u32,
                move_speed_ms: u64::from(template.move_speed),
                attack_speed_ms: u64::from(template.attack_speed),
                friendly_guild: None,
                position: position.clone(),
                direction: *direction,
                defense: ZoneMonsterDefense::from_crystal_template(&template),
                drops: Vec::new(),
            }
        })
        .collect())
}

fn director_map_file_name(zone_id: &str) -> Result<&str, String> {
    zone_id
        .strip_prefix("map:")
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("director Zone {zone_id} is not a map Zone"))
}

fn simulation_checkpoint_commitment(
    checkpoint: &DirectorSimulationCheckpoint,
) -> Result<String, String> {
    let mut unsigned = checkpoint.clone();
    unsigned.state_commitment.clear();
    let bytes = domain_json(DIRECTOR_SIMULATION_CHECKPOINT_DOMAIN, &unsigned)?;
    Ok(hex_digest(Sha256::digest(bytes)))
}

#[derive(Debug, Clone)]
pub struct ZoneDirectorExecutor {
    zone_host_id: String,
    trusted_director: String,
    identity: NodeSigningIdentity,
    guard: DirectorReplayGuard,
}

impl ZoneDirectorExecutor {
    pub fn new(
        zone_host_id: impl Into<String>,
        trusted_director: impl Into<String>,
        identity: NodeSigningIdentity,
    ) -> Result<Self, String> {
        let zone_host_id = zone_host_id.into();
        validate_component("Zone Host id", &zone_host_id)?;
        Ok(Self {
            zone_host_id,
            trusted_director: trusted_director.into(),
            identity,
            guard: DirectorReplayGuard::default(),
        })
    }

    /// Converts a finalized world command into deterministic, scheduled Zone
    /// intents. Concrete simulation workers consume these intents; the director
    /// never receives a database or asset-minting capability.
    pub fn execute(
        &mut self,
        command: &SignedDirectorCommand,
        control_height: u64,
        now_ms: u64,
    ) -> Result<DirectorExecutionReceipt, String> {
        if let Some(receipt) = self.guard.receipt(&command.payload.command_id) {
            return Ok(receipt.clone());
        }
        command.verify(&self.trusted_director, now_ms)?;
        if control_height == 0 {
            return Err("director command requires a finalized control height".to_string());
        }
        let applied_stages = command
            .payload
            .stages
            .iter()
            .map(|stage| AppliedDirectorStage {
                stage_id: stage.stage_id.clone(),
                scheduled_at_ms: now_ms.saturating_add(stage.starts_after_ms),
                action_count: stage.actions.len(),
            })
            .collect::<Vec<_>>();
        let commitment = execution_commitment(
            &command.payload.command_id,
            control_height,
            &self.zone_host_id,
            &applied_stages,
        )?;
        let mut receipt = DirectorExecutionReceipt {
            schema: WORLD_DIRECTOR_SCHEMA.to_string(),
            command_id: command.payload.command_id.clone(),
            control_height,
            zone_host_id: self.zone_host_id.clone(),
            accepted: true,
            applied_stages,
            state_commitment: commitment,
            executed_at_ms: now_ms,
            executor_public_key: self.identity.public_key().to_string(),
            signature_algorithm: "ed25519-zip215".to_string(),
            signature: String::new(),
        };
        receipt.signature = self.identity.sign(&receipt.signing_bytes()?);
        self.guard
            .applied
            .insert(command.payload.command_id.clone(), receipt.clone());
        Ok(receipt)
    }
}

fn execution_commitment(
    command_id: &str,
    control_height: u64,
    zone_host_id: &str,
    stages: &[AppliedDirectorStage],
) -> Result<String, String> {
    let bytes = serde_json::to_vec(&(command_id, control_height, zone_host_id, stages))
        .map_err(|error| format!("director execution commitment encode failed: {error}"))?;
    Ok(hex_digest(Sha256::digest(bytes)))
}

fn domain_json(domain: &[u8], value: &impl Serialize) -> Result<Vec<u8>, String> {
    let payload = serde_json::to_vec(value)
        .map_err(|error| format!("world director canonical serialization failed: {error}"))?;
    let mut bytes = Vec::with_capacity(domain.len() + payload.len());
    bytes.extend_from_slice(domain);
    bytes.extend_from_slice(&payload);
    Ok(bytes)
}

fn validate_allowed_zone(zone_id: &str, allowed_zones: &BTreeSet<String>) -> Result<(), String> {
    validate_component("director zone id", zone_id)?;
    if !allowed_zones.contains(zone_id) {
        return Err(format!("director action targets disallowed zone {zone_id}"));
    }
    Ok(())
}

fn validate_component(label: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() || value.len() > 255 || value.chars().any(char::is_control) {
        return Err(format!("invalid {label}"));
    }
    Ok(())
}

fn validate_text(label: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty()
        || value.len() > MAX_DIRECTOR_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(format!("invalid {label}"));
    }
    Ok(())
}

fn bps(numerator: u64, denominator: u64) -> u64 {
    numerator
        .saturating_mul(BASIS_POINTS)
        .checked_div(denominator.max(1))
        .unwrap_or_default()
}

fn clamp_bps(value: u64) -> u16 {
    value.min(BASIS_POINTS) as u16
}

fn stable_u64(value: &str) -> u64 {
    let digest = Sha256::digest(value.as_bytes());
    u64::from_be_bytes(
        digest[..8]
            .try_into()
            .expect("SHA-256 prefix must contain eight bytes"),
    )
}

fn hex_digest(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consensus_log::CommonwareControlLog;

    static CHECKPOINT_TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    struct CheckpointTestDirectory(PathBuf);

    impl CheckpointTestDirectory {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "mir2-world-director-checkpoint-{label}-{}-{}",
                std::process::id(),
                CHECKPOINT_TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn checkpoint(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for CheckpointTestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn checkpoint_test_committee() -> Vec<String> {
        [
            "validator-a".to_string(),
            "validator-b".to_string(),
            "validator-c".to_string(),
            "validator-d".to_string(),
        ]
        .to_vec()
    }

    fn checkpoint_test_service(
        path: PathBuf,
    ) -> (
        Vec<String>,
        String,
        Arc<SharedInProcessZoneRuntimeFactory>,
        WorldDirectorRuntimeService,
    ) {
        let committee = checkpoint_test_committee();
        let director = NodeSigningIdentity::from_seed([91; 32])
            .public_key()
            .to_string();
        let factory = Arc::new(SharedInProcessZoneRuntimeFactory::new());
        let service = WorldDirectorRuntimeService::new(
            committee.clone(),
            director.clone(),
            Arc::clone(&factory),
            Some(path),
        )
        .unwrap();
        (committee, director, factory, service)
    }

    fn checkpoint_startup_error(path: PathBuf) -> String {
        let director = NodeSigningIdentity::from_seed([92; 32]);
        WorldDirectorRuntimeService::new(
            checkpoint_test_committee(),
            director.public_key(),
            Arc::new(SharedInProcessZoneRuntimeFactory::new()),
            Some(path),
        )
        .err()
        .expect("an existing invalid checkpoint must fail startup")
    }

    fn snapshot() -> WorldTelemetrySnapshot {
        WorldTelemetrySnapshot {
            schema: WORLD_DIRECTOR_SCHEMA.to_string(),
            snapshot_id: "snapshot-wooma-001".to_string(),
            game_id: "mir2".to_string(),
            region_id: "asia-hk".to_string(),
            observed_at_ms: 1_000_000,
            window_ms: 15 * 60 * 1_000,
            maps: vec![
                MapTelemetrySnapshot {
                    zone_id: "map:0".to_string(),
                    active_players: 80,
                    median_level: 18,
                    new_player_count: 20,
                    returning_player_count: 5,
                    monster_kills: 8_000,
                    boss_kills: 8,
                    player_deaths: 20,
                    completed_quests: 50,
                },
                MapTelemetrySnapshot {
                    zone_id: "map:D022".to_string(),
                    active_players: 20,
                    median_level: 26,
                    new_player_count: 0,
                    returning_player_count: 0,
                    monster_kills: 4_000,
                    boss_kills: 4,
                    player_deaths: 15,
                    completed_quests: 15,
                },
                MapTelemetrySnapshot {
                    zone_id: "map:D023".to_string(),
                    active_players: 12,
                    median_level: 29,
                    new_player_count: 0,
                    returning_player_count: 0,
                    monster_kills: 3_000,
                    boss_kills: 6,
                    player_deaths: 12,
                    completed_quests: 8,
                },
                MapTelemetrySnapshot {
                    zone_id: "map:D024".to_string(),
                    active_players: 8,
                    median_level: 31,
                    new_player_count: 0,
                    returning_player_count: 0,
                    monster_kills: 1_000,
                    boss_kills: 12,
                    player_deaths: 30,
                    completed_quests: 2,
                },
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

    fn approved(
        now_ms: u64,
    ) -> (
        WorldTelemetrySnapshot,
        ApprovedDirectorPlan,
        NodeSigningIdentity,
    ) {
        let snapshot = snapshot();
        let scores = DirectorPressureScores::from_snapshot(&snapshot).unwrap();
        let proposal = DirectorProposal::bichon_wooma_rule(&snapshot, &scores, now_ms).unwrap();
        let plan = WorldDirectorPolicy::mir2_default()
            .approve(
                &proposal,
                &snapshot,
                &scores,
                &DirectorPolicyState::default(),
                now_ms,
            )
            .unwrap();
        (snapshot, plan, NodeSigningIdentity::from_seed([31; 32]))
    }

    #[test]
    fn pressure_model_is_bounded_and_deterministic() {
        let first = DirectorPressureScores::from_snapshot(&snapshot()).unwrap();
        let second = DirectorPressureScores::from_snapshot(&snapshot()).unwrap();
        assert_eq!(first, second);
        assert!(first.content_fatigue_bps >= 5_000);
        assert!(first.guild_dominance_bps <= 10_000);
    }

    #[test]
    fn policy_rejects_unobserved_zone_and_excess_budget() {
        let snapshot = snapshot();
        let scores = DirectorPressureScores::from_snapshot(&snapshot).unwrap();
        let mut proposal =
            DirectorProposal::bichon_wooma_rule(&snapshot, &scores, 1_000_001).unwrap();
        proposal.target_zones.insert("map:secret".to_string());
        assert!(WorldDirectorPolicy::mir2_default()
            .approve(
                &proposal,
                &snapshot,
                &scores,
                &DirectorPolicyState::default(),
                1_000_001,
            )
            .is_err());
        proposal.target_zones.remove("map:secret");
        proposal.reward_budget = u64::MAX;
        assert!(WorldDirectorPolicy::mir2_default()
            .approve(
                &proposal,
                &snapshot,
                &scores,
                &DirectorPolicyState::default(),
                1_000_001,
            )
            .is_err());
    }

    #[test]
    fn signed_command_rejects_tamper_and_expiry() {
        let now_ms = 1_000_001;
        let (snapshot, plan, director) = approved(now_ms);
        let command = SignedDirectorCommand::issue(&plan, &snapshot, &director, 60_000).unwrap();
        command.verify(director.public_key(), now_ms).unwrap();
        let mut tampered = command.clone();
        tampered.payload.reward_budget += 1;
        assert!(tampered.verify(director.public_key(), now_ms).is_err());
        assert!(command
            .verify(director.public_key(), now_ms + 60_001)
            .is_err());
    }

    #[test]
    fn ai_adapter_accepts_only_strict_bounded_template_proposals() {
        let snapshot = snapshot();
        let scores = DirectorPressureScores::from_snapshot(&snapshot).unwrap();
        let policy = WorldDirectorPolicy::mir2_default();
        let request = policy.ai_request(&snapshot, &scores).unwrap();
        assert_eq!(request.templates.len(), 1);
        assert!(!request.instruction.contains("database"));

        let rule = DirectorProposal::bichon_wooma_rule(&snapshot, &scores, 1_000_001).unwrap();
        let ai = DirectorProposal {
            source: DirectorProposalSource::Ai {
                provider: "model-gateway".to_string(),
                model: "world-director-small".to_string(),
            },
            ..rule
        };
        let adapter =
            AiDirectorProposalAdapter::new("model-gateway", "world-director-small").unwrap();
        let decoded = adapter.decode(&serde_json::to_vec(&ai).unwrap()).unwrap();
        policy
            .approve(
                &decoded,
                &snapshot,
                &scores,
                &DirectorPolicyState::default(),
                1_000_001,
            )
            .unwrap();

        let mut value = serde_json::to_value(&ai).unwrap();
        value["directDatabaseWrite"] = serde_json::json!({"gold": 999999});
        assert!(adapter
            .decode(&serde_json::to_vec(&value).unwrap())
            .is_err());
    }

    #[test]
    fn bichon_wooma_command_finalizes_and_executes_idempotently() {
        let now_ms = 1_000_001;
        let (snapshot, plan, director) = approved(now_ms);
        let command = SignedDirectorCommand::issue(&plan, &snapshot, &director, 60_000).unwrap();
        let control = CommonwareControlLog::new([
            "validator-a".to_string(),
            "validator-b".to_string(),
            "validator-c".to_string(),
            "validator-d".to_string(),
        ])
        .unwrap();
        let block = control
            .propose("validator-a", vec![command.control_envelope().unwrap()])
            .unwrap();
        assert!(control
            .vote("validator-a", &block.digest)
            .unwrap()
            .is_none());
        assert!(control
            .vote("validator-b", &block.digest)
            .unwrap()
            .is_none());
        let finalized = control
            .vote("validator-c", &block.digest)
            .unwrap()
            .expect("three of four validators should finalize");
        let decoded =
            director_commands_from_finalized(&finalized, director.public_key(), now_ms).unwrap();
        assert_eq!(decoded, vec![command.clone()]);

        let zone_identity = NodeSigningIdentity::from_seed([41; 32]);
        let mut executor = ZoneDirectorExecutor::new(
            "zone-host-hk-1",
            director.public_key(),
            zone_identity.clone(),
        )
        .unwrap();
        let first = executor
            .execute(&decoded[0], finalized.block.height, now_ms)
            .unwrap();
        let replay = executor
            .execute(&decoded[0], finalized.block.height, now_ms + 1)
            .unwrap();
        assert_eq!(first, replay);
        assert_eq!(first.applied_stages.len(), 3);
        first.verify(zone_identity.public_key()).unwrap();
    }

    #[test]
    fn finalized_schedule_spawns_real_zone_monsters_and_survives_restart() {
        let now_ms = 1_000_001;
        let (snapshot, plan, director) = approved(now_ms);
        let command = SignedDirectorCommand::issue(&plan, &snapshot, &director, 60_000).unwrap();
        let factory = SharedInProcessZoneRuntimeFactory::new();
        let mut adapter = Mir2DirectorSimulationAdapter::new(director.public_key()).unwrap();
        assert!(adapter.install(command.clone(), now_ms).unwrap());

        let opening = adapter.advance(now_ms, &factory).unwrap();
        assert_eq!(opening.spawned_monsters, 0);
        assert_eq!(opening.broadcast_messages, 8);

        let checkpoint = adapter.checkpoint_bytes().unwrap();
        let mut restored =
            Mir2DirectorSimulationAdapter::restore(&checkpoint, director.public_key()).unwrap();
        let incursion = restored.advance(now_ms + 5 * 60 * 1_000, &factory).unwrap();
        assert_eq!(incursion.spawned_monsters, 24);
        assert_eq!(
            factory
                .world_event_monster_count(&ZoneId::new("map:D022"), "D022")
                .unwrap(),
            24
        );
        let incursion_monsters = factory
            .world_event_monster_snapshots(&ZoneId::new("map:D022"), "D022")
            .unwrap();
        let unique_positions = incursion_monsters
            .iter()
            .map(|monster| (monster.position.x, monster.position.y))
            .collect::<BTreeSet<_>>();
        assert_eq!(unique_positions.len(), 24);
        let base_positions = crystal_map_respawns_by_file_name("D022")
            .unwrap()
            .respawns
            .iter()
            .flat_map(|respawn| crystal_world_respawn_spawns("D022", respawn))
            .map(|(_, position, _)| (position.x, position.y))
            .collect::<BTreeSet<_>>();
        assert!(
            unique_positions.is_disjoint(&base_positions),
            "event monsters must not overlap Crystal's ordinary monster slots"
        );
        let min_x = unique_positions.iter().map(|(x, _)| *x).min().unwrap();
        let max_x = unique_positions.iter().map(|(x, _)| *x).max().unwrap();
        let min_y = unique_positions.iter().map(|(_, y)| *y).min().unwrap();
        let max_y = unique_positions.iter().map(|(_, y)| *y).max().unwrap();
        assert!(
            max_x - min_x > 20 || max_y - min_y > 20,
            "event monsters must use Crystal's walkable respawn spread instead of a dense 5x5 cluster"
        );

        let replay = restored.advance(now_ms + 5 * 60 * 1_000, &factory).unwrap();
        assert_eq!(replay.spawned_monsters, 0);
        assert!(replay.applied_action_keys.is_empty());

        let finale = restored
            .advance(now_ms + 20 * 60 * 1_000, &factory)
            .unwrap();
        assert_eq!(finale.spawned_monsters, 1);
        assert_eq!(
            factory
                .world_event_monster_count(&ZoneId::new("map:D024"), "D024")
                .unwrap(),
            1
        );

        let single_factory = SharedInProcessZoneRuntimeFactory::new();
        let mut single = Mir2DirectorSimulationAdapter::new(director.public_key()).unwrap();
        single.install(command, now_ms).unwrap();
        let single_receipt = single
            .advance_with_zone_router(now_ms + 20 * 60 * 1_000, &single_factory, |_| {
                ZoneId::primary()
            })
            .unwrap();
        assert_eq!(single_receipt.spawned_monsters, 25);
        assert_eq!(
            single_factory
                .world_event_monster_count(&ZoneId::primary(), "D022")
                .unwrap(),
            24
        );
    }

    #[test]
    fn runtime_service_imports_finality_and_restores_pending_stages() {
        let now_ms = 1_000_001;
        let (snapshot, plan, director) = approved(now_ms);
        let command = SignedDirectorCommand::issue(&plan, &snapshot, &director, 60_000).unwrap();
        let validator_identities = [
            NodeSigningIdentity::from_seed([31; 32]),
            NodeSigningIdentity::from_seed([32; 32]),
            NodeSigningIdentity::from_seed([33; 32]),
            NodeSigningIdentity::from_seed([34; 32]),
        ];
        let committee = validator_identities
            .iter()
            .map(|validator| validator.public_key().to_string())
            .collect::<Vec<_>>();
        let producer = CommonwareControlLog::new(committee.clone()).unwrap();
        let block = producer
            .propose(&committee[0], vec![command.control_envelope().unwrap()])
            .unwrap();
        producer.vote(&committee[0], &block.digest).unwrap();
        producer.vote(&committee[1], &block.digest).unwrap();
        let finalized = producer
            .vote(&committee[2], &block.digest)
            .unwrap()
            .expect("quorum should finalize");
        let submission =
            FinalizedDirectorSubmission::issue(finalized, &validator_identities).unwrap();

        let checkpoint_path = std::env::temp_dir().join(format!(
            "mir2-world-director-runtime-{}.json",
            std::process::id()
        ));
        let factory = Arc::new(SharedInProcessZoneRuntimeFactory::new());
        let service = WorldDirectorRuntimeService::new(
            committee.clone(),
            director.public_key(),
            Arc::clone(&factory),
            Some(checkpoint_path.clone()),
        )
        .unwrap();
        let mut forged = submission.clone();
        *forged
            .validator_signatures
            .values_mut()
            .next()
            .expect("submission has quorum signatures") = "forged".to_string();
        assert!(service.install_submission(forged, now_ms).is_err());
        let installed = service
            .install_submission(submission.clone(), now_ms)
            .unwrap();
        assert!(installed.accepted);
        assert_eq!(installed.newly_installed_commands, 1);
        assert_eq!(installed.advance.broadcast_messages, 8);
        let replay = service.install_submission(submission, now_ms + 1).unwrap();
        assert!(!replay.accepted);
        let running_status = service.status().unwrap();
        assert_eq!(running_status.finalized_height, 1);
        assert!(running_status.checkpoint.configured);
        assert_eq!(running_status.checkpoint.write_attempts_total, 2);
        assert_eq!(running_status.checkpoint.writes_total, 2);
        assert_eq!(running_status.checkpoint.write_failures_total, 0);
        assert!(running_status.checkpoint.file_bytes > 0);
        assert!(running_status.checkpoint.last_zone_factory_bytes > 0);
        assert_eq!(running_status.checkpoint.last_success_at_ms, now_ms + 1);

        drop(service);
        drop(factory);
        let restored_factory = Arc::new(SharedInProcessZoneRuntimeFactory::new());
        let restored = WorldDirectorRuntimeService::new(
            committee.clone(),
            director.public_key(),
            Arc::clone(&restored_factory),
            Some(checkpoint_path.clone()),
        )
        .unwrap();
        let restored_status = restored.status().unwrap();
        assert_eq!(restored_status.installed_command_count, 1);
        assert!(restored_status.checkpoint.configured);
        assert!(restored_status.checkpoint.file_bytes > 0);
        assert_eq!(restored_status.checkpoint.write_attempts_total, 0);
        assert_eq!(restored_status.checkpoint.writes_total, 0);
        assert_eq!(restored_status.checkpoint.last_success_at_ms, now_ms + 1);
        let incursion = restored.advance(now_ms + 5 * 60 * 1_000).unwrap();
        assert_eq!(incursion.spawned_monsters, 24);
        let status = restored.status().unwrap();
        assert_eq!(status.spawned_monsters_total, 24);
        assert_eq!(status.world_event_monsters_by_zone["map:D022"], 24);
        assert_eq!(status.checkpoint.write_attempts_total, 1);
        assert_eq!(status.checkpoint.writes_total, 1);
        assert_eq!(status.checkpoint.write_failures_total, 0);
        assert!(status.checkpoint.last_zone_factory_bytes > 0);
        assert_eq!(
            status.checkpoint.last_success_at_ms,
            now_ms + 5 * 60 * 1_000
        );
        assert_eq!(
            restored_factory
                .world_event_monster_count(&ZoneId::new("map:D022"), "D022")
                .unwrap(),
            24
        );
        drop(restored);
        drop(restored_factory);
        let restarted_factory = Arc::new(SharedInProcessZoneRuntimeFactory::new());
        let restarted = WorldDirectorRuntimeService::new(
            committee,
            director.public_key(),
            Arc::clone(&restarted_factory),
            Some(checkpoint_path.clone()),
        )
        .unwrap();
        assert_eq!(
            restarted.status().unwrap().world_event_monsters_by_zone["map:D022"],
            24
        );
        std::fs::remove_file(checkpoint_path).unwrap();
    }

    #[test]
    fn runtime_service_allows_an_explicitly_configured_missing_checkpoint() {
        let directory = CheckpointTestDirectory::new("missing");
        let checkpoint_path = directory.checkpoint("checkpoint.json");
        let (_, _, _, service) = checkpoint_test_service(checkpoint_path.clone());

        assert!(!checkpoint_path.exists());
        let status = service.status().unwrap();
        assert!(status.checkpoint.configured);
        assert_eq!(status.checkpoint.file_bytes, 0);
        assert_eq!(status.installed_command_count, 0);
        assert_eq!(status.spawned_monsters_total, 0);

        let nested_checkpoint = directory
            .0
            .join("missing-parent")
            .join("missing-child")
            .join("checkpoint.json");
        let (_, _, _, nested_service) = checkpoint_test_service(nested_checkpoint.clone());
        assert!(!nested_checkpoint.exists());
        assert!(!directory.0.join("missing-parent").exists());
        assert!(nested_service.status().unwrap().checkpoint.configured);
    }

    #[test]
    fn runtime_service_fails_closed_for_corrupt_or_incompatible_checkpoint() {
        let directory = CheckpointTestDirectory::new("invalid");
        let corrupt = directory.checkpoint("corrupt.json");
        fs::write(&corrupt, b"{not-json").unwrap();
        let corrupt_error = checkpoint_startup_error(corrupt);
        assert!(corrupt_error.contains("checkpoint decode failed"));

        let incompatible = directory.checkpoint("incompatible.json");
        fs::write(&incompatible, br#"{"version":2}"#).unwrap();
        let incompatible_error = checkpoint_startup_error(incompatible);
        assert!(incompatible_error.contains("is not the current format"));
    }

    #[test]
    fn runtime_service_fails_closed_before_reading_an_oversized_checkpoint() {
        let directory = CheckpointTestDirectory::new("oversized");
        let checkpoint_path = directory.checkpoint("checkpoint.json");
        let file = File::create(&checkpoint_path).unwrap();
        file.set_len(MAX_DIRECTOR_RUNTIME_CHECKPOINT_BYTES + 1)
            .unwrap();
        drop(file);

        let error = checkpoint_startup_error(checkpoint_path);
        assert!(error.contains("limit"));
        assert!(error.contains(&(MAX_DIRECTOR_RUNTIME_CHECKPOINT_BYTES + 1).to_string()));
    }

    #[cfg(unix)]
    #[test]
    fn runtime_service_rejects_a_symbolic_link_checkpoint() {
        use std::os::unix::fs::symlink;

        let directory = CheckpointTestDirectory::new("symlink");
        let target = directory.checkpoint("target.json");
        let checkpoint_path = directory.checkpoint("checkpoint.json");
        fs::write(&target, br#"{"version":3}"#).unwrap();
        symlink(&target, &checkpoint_path).unwrap();

        let error = checkpoint_startup_error(checkpoint_path);
        assert!(error.contains("not a regular file") || error.contains("without following links"));

        let real_parent = directory.checkpoint("real-parent");
        let linked_parent = directory.checkpoint("linked-parent");
        fs::create_dir(&real_parent).unwrap();
        fs::write(real_parent.join("checkpoint.json"), br#"{"version":3}"#).unwrap();
        symlink(&real_parent, &linked_parent).unwrap();
        let parent_error = checkpoint_startup_error(linked_parent.join("checkpoint.json"));
        assert!(parent_error.contains("symbolic-link component"));
    }

    #[cfg(windows)]
    #[test]
    fn runtime_service_rejects_a_windows_symlink_checkpoint_when_supported() {
        use std::os::windows::fs::symlink_file;

        let directory = CheckpointTestDirectory::new("windows-symlink");
        let target = directory.checkpoint("target.json");
        let checkpoint_path = directory.checkpoint("checkpoint.json");
        fs::write(&target, br#"{"version":3}"#).unwrap();
        match symlink_file(&target, &checkpoint_path) {
            Ok(()) => {
                let error = checkpoint_startup_error(checkpoint_path);
                assert!(
                    error.contains("not a regular file")
                        || error.contains("reparse point")
                        || error.contains("without following links")
                );
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::PermissionDenied | io::ErrorKind::Unsupported
                ) => {}
            Err(error) => panic!("unexpected Windows symlink creation failure: {error}"),
        }
    }

    #[test]
    fn checkpoint_read_rejects_an_ancestor_replacement_race() {
        let directory = CheckpointTestDirectory::new("read-ancestor-race");
        let guarded_directory = directory.checkpoint("guarded");
        let replacement_directory = directory.checkpoint("replacement");
        let parked_directory = directory.checkpoint("parked");
        fs::create_dir(&guarded_directory).unwrap();
        fs::create_dir(&replacement_directory).unwrap();
        let checkpoint_path = guarded_directory.join("checkpoint.json");
        let (_, _, factory, service) = checkpoint_test_service(checkpoint_path.clone());
        {
            let state = service.state.lock().unwrap();
            persist_runtime_checkpoint(&checkpoint_path, &state, factory.as_ref()).unwrap();
        }
        let checkpoint_bytes = fs::read(&checkpoint_path).unwrap();
        fs::write(
            replacement_directory.join("checkpoint.json"),
            &checkpoint_bytes,
        )
        .unwrap();

        let mut attempted = false;
        let error = read_runtime_checkpoint_bytes_with_callback(&checkpoint_path, |phase| {
            if phase != DirectorCheckpointReadPhase::AncestorsGuarded {
                return Ok(());
            }
            attempted = true;
            fs::rename(&guarded_directory, &parked_directory).map_err(|error| {
                format!("ancestor replacement was blocked by the held directory guard: {error}")
            })?;
            fs::rename(&replacement_directory, &guarded_directory).map_err(|error| {
                format!("failed to install the replacement checkpoint directory: {error}")
            })?;
            Ok(())
        })
        .expect_err("an ancestor replacement must never yield checkpoint bytes");
        assert!(attempted);
        assert!(
            error.contains("ancestor path identity changed")
                || error.contains("blocked by the held directory guard")
        );
    }

    #[test]
    fn checkpoint_publish_rejects_an_ancestor_replacement_and_preserves_old_bytes() {
        let directory = CheckpointTestDirectory::new("write-ancestor-race");
        let guarded_directory = directory.checkpoint("guarded");
        let replacement_directory = directory.checkpoint("replacement");
        let parked_directory = directory.checkpoint("parked");
        fs::create_dir(&guarded_directory).unwrap();
        fs::create_dir(&replacement_directory).unwrap();
        let checkpoint_path = guarded_directory.join("checkpoint.json");
        let (_, _, factory, service) = checkpoint_test_service(checkpoint_path.clone());
        let mut state = service.state.lock().unwrap();
        state.spawned_monsters_total = 11;
        persist_runtime_checkpoint(&checkpoint_path, &state, factory.as_ref()).unwrap();
        let old_bytes = fs::read(&checkpoint_path).unwrap();
        fs::write(replacement_directory.join("checkpoint.json"), &old_bytes).unwrap();

        state.spawned_monsters_total = 22;
        let mut attempted = false;
        let error = persist_runtime_checkpoint_with_callback(
            &checkpoint_path,
            &state,
            factory.as_ref(),
            |phase| {
                if phase != DirectorCheckpointPublishPhase::BeforeRename {
                    return Ok(());
                }
                attempted = true;
                fs::rename(&guarded_directory, &parked_directory).map_err(|error| {
                    format!("ancestor replacement was blocked by the held directory guard: {error}")
                })?;
                fs::rename(&replacement_directory, &guarded_directory).map_err(|error| {
                    format!("failed to install the replacement checkpoint directory: {error}")
                })?;
                Ok(())
            },
        )
        .err()
        .expect("an ancestor replacement must reject checkpoint publication");
        assert!(attempted);
        assert!(
            error.contains("ancestor path identity changed")
                || error.contains("blocked by the held directory guard")
        );
        assert_eq!(fs::read(&checkpoint_path).unwrap(), old_bytes);
        if parked_directory.exists() {
            assert_eq!(
                fs::read(parked_directory.join("checkpoint.json")).unwrap(),
                old_bytes
            );
        }
    }

    #[test]
    fn checkpoint_publication_faults_leave_a_complete_old_or_new_image() {
        let directory = CheckpointTestDirectory::new("faults");
        let seed_path = directory.checkpoint("seed.json");
        let (committee, trusted_director, factory, service) =
            checkpoint_test_service(seed_path.clone());
        let mut state = service.state.lock().unwrap();
        state.spawned_monsters_total = 11;
        state.last_checkpoint_at_ms = 11;
        persist_runtime_checkpoint(&seed_path, &state, factory.as_ref()).unwrap();
        let old_bytes = fs::read(&seed_path).unwrap();
        state.spawned_monsters_total = 22;
        state.last_checkpoint_at_ms = 22;

        let phases = [
            DirectorCheckpointPublishPhase::TempCreated,
            DirectorCheckpointPublishPhase::TempWritten,
            DirectorCheckpointPublishPhase::TempSynced,
            DirectorCheckpointPublishPhase::BeforeRename,
            DirectorCheckpointPublishPhase::Renamed,
            DirectorCheckpointPublishPhase::BeforeDirectorySync,
            DirectorCheckpointPublishPhase::DirectorySynced,
        ];
        for (index, injected_phase) in phases.into_iter().enumerate() {
            let checkpoint_path = directory.checkpoint(&format!("phase-{index}.json"));
            fs::write(&checkpoint_path, &old_bytes).unwrap();
            let error = persist_runtime_checkpoint_with_callback(
                &checkpoint_path,
                &state,
                factory.as_ref(),
                |observed_phase| {
                    if observed_phase == injected_phase {
                        Err(format!("injected checkpoint fault at {observed_phase:?}"))
                    } else {
                        Ok(())
                    }
                },
            )
            .err()
            .expect("the selected publication phase must inject a failure");
            assert!(error.contains("injected checkpoint fault"));

            let checkpoint_bytes = fs::read(&checkpoint_path).unwrap();
            if matches!(
                injected_phase,
                DirectorCheckpointPublishPhase::TempCreated
                    | DirectorCheckpointPublishPhase::TempWritten
                    | DirectorCheckpointPublishPhase::TempSynced
                    | DirectorCheckpointPublishPhase::BeforeRename
            ) {
                assert_eq!(
                    checkpoint_bytes, old_bytes,
                    "phase {injected_phase:?} must preserve every byte of the old checkpoint"
                );
            }

            let restored_factory = SharedInProcessZoneRuntimeFactory::new();
            let restored = restore_runtime_checkpoint(
                &checkpoint_path,
                &committee,
                &trusted_director,
                &restored_factory,
            )
            .unwrap()
            .expect("fault injection must leave a checkpoint path")
            .0;
            assert!(
                matches!(restored.spawned_monsters_total, 11 | 22),
                "phase {injected_phase:?} produced neither the old nor new complete checkpoint"
            );
            if matches!(
                injected_phase,
                DirectorCheckpointPublishPhase::TempCreated
                    | DirectorCheckpointPublishPhase::TempWritten
                    | DirectorCheckpointPublishPhase::TempSynced
                    | DirectorCheckpointPublishPhase::BeforeRename
            ) {
                assert_eq!(restored.spawned_monsters_total, 11);
            } else {
                assert_eq!(restored.spawned_monsters_total, 22);
            }
            let leaked_temp = fs::read_dir(&directory.0).unwrap().any(|entry| {
                entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .ends_with(".tmp")
            });
            #[cfg(windows)]
            assert!(!leaked_temp, "phase {injected_phase:?} leaked a temp file");
            #[cfg(not(windows))]
            let _ = leaked_temp;
        }
    }

    #[test]
    fn runtime_service_fails_startup_for_non_directory_checkpoint_parent() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let checkpoint_parent = std::env::temp_dir().join(format!(
            "mir2-world-director-checkpoint-parent-{}-{nonce}",
            std::process::id()
        ));
        std::fs::write(&checkpoint_parent, b"not-a-directory").unwrap();
        let checkpoint_path = checkpoint_parent.join("checkpoint.json");
        let director = NodeSigningIdentity::from_seed([71; 32]);
        let error = WorldDirectorRuntimeService::new(
            [
                "validator-a".to_string(),
                "validator-b".to_string(),
                "validator-c".to_string(),
                "validator-d".to_string(),
            ],
            director.public_key(),
            Arc::new(SharedInProcessZoneRuntimeFactory::new()),
            Some(checkpoint_path),
        )
        .err()
        .expect("a file where the checkpoint directory belongs must fail startup");
        assert!(error.contains("non-directory or symbolic-link component"));

        std::fs::remove_file(checkpoint_parent).unwrap();
    }

    #[test]
    fn pre_existing_checkpoint_temp_is_never_adopted_or_removed() {
        let directory = CheckpointTestDirectory::new("pre-existing-temp");
        let temporary = directory.checkpoint("checkpoint.pre-existing.tmp");
        let original_bytes = b"pre-existing-temp-must-survive";
        fs::write(&temporary, original_bytes).unwrap();

        let error = create_director_checkpoint_temp(&temporary)
            .err()
            .expect("create_new must reject a pre-existing temporary path");
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&temporary).unwrap(), original_bytes);
    }

    #[cfg(windows)]
    #[test]
    fn windows_temp_handle_blocks_replacement_and_cleanup_deletes_only_created_object() {
        let directory = CheckpointTestDirectory::new("temp-handle-cleanup");
        let temporary = directory.checkpoint("checkpoint.created.tmp");
        let replacement = directory.checkpoint("replacement-candidate.tmp");
        let replacement_bytes = b"replacement-must-not-be-deleted";
        fs::write(&replacement, replacement_bytes).unwrap();
        let created = create_director_checkpoint_temp(&temporary).unwrap();
        let ancestors = DirectorCheckpointAncestorGuard::capture(&directory.0).unwrap();

        OpenOptions::new()
            .write(true)
            .open(&temporary)
            .expect_err("the verified temporary handle must deny a second writer");
        let _replace_error = fs::rename(&replacement, &temporary)
            .expect_err("the verified temporary handle must deny path replacement");
        assert_eq!(fs::read(&replacement).unwrap(), replacement_bytes);
        assert!(temporary.exists());

        safely_remove_director_checkpoint_temp(&temporary, &ancestors, created).unwrap();
        assert!(!temporary.exists());
        assert_eq!(fs::read(&replacement).unwrap(), replacement_bytes);
    }

    #[cfg(unix)]
    #[test]
    fn unix_temp_cleanup_refuses_to_delete_a_replacement_path() {
        let directory = CheckpointTestDirectory::new("temp-replacement-cleanup");
        let temporary = directory.checkpoint("checkpoint.created.tmp");
        let parked = directory.checkpoint("checkpoint.created.parked");
        let replacement_bytes = b"replacement-must-survive";
        let created = create_director_checkpoint_temp(&temporary).unwrap();
        let ancestors = DirectorCheckpointAncestorGuard::capture(&directory.0).unwrap();
        fs::rename(&temporary, &parked).unwrap();
        fs::write(&temporary, replacement_bytes).unwrap();

        let error = safely_remove_director_checkpoint_temp(&temporary, &ancestors, created)
            .expect_err("path replacement must make cleanup fail closed");
        assert!(error.contains("path was replaced"));
        assert_eq!(fs::read(&temporary).unwrap(), replacement_bytes);
        assert!(parked.exists());
    }
}
