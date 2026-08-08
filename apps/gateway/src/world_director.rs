use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

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
const DIRECTOR_RUNTIME_CHECKPOINT_VERSION: u32 = 2;
const DIRECTOR_RUNTIME_CHECKPOINT_DOMAIN: &[u8] = b"obelisk.world-director.runtime-checkpoint.v1\0";
const DIRECTOR_RUNTIME_CHECKPOINT_INTERVAL_MS: u64 = 30_000;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorldDirectorRuntimeCheckpoint {
    version: u32,
    finalized: Vec<FinalizedControlBlock>,
    simulation_checkpoint: Vec<u8>,
    zone_factory_checkpoint: Vec<u8>,
    spawned_monsters_total: u64,
    broadcast_messages_total: u64,
    last_advance_at_ms: u64,
    last_checkpoint_at_ms: u64,
    last_advance: Option<DirectorSimulationAdvanceReceipt>,
    state_commitment: String,
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
        if let Some(path) = checkpoint_path.as_deref().filter(|path| path.exists()) {
            state = restore_runtime_checkpoint(
                path,
                &committee.iter().cloned().collect::<Vec<_>>(),
                &trusted_director,
                factory.as_ref(),
            )?;
        }
        Ok(Self {
            committee,
            trusted_director,
            factory,
            runtime_zone_id,
            checkpoint_path,
            state: Mutex::new(state),
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
            state_commitment: state.adapter.state_commitment()?,
        })
    }

    fn persist_locked(&self, state: &WorldDirectorRuntimeState) -> Result<(), String> {
        let Some(path) = self.checkpoint_path.as_deref() else {
            return Ok(());
        };
        persist_runtime_checkpoint(path, state, self.factory.as_ref())
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

fn restore_runtime_checkpoint(
    path: &Path,
    committee: &[String],
    trusted_director: &str,
    factory: &SharedInProcessZoneRuntimeFactory,
) -> Result<WorldDirectorRuntimeState, String> {
    let bytes = fs::read(path).map_err(|error| {
        format!(
            "failed to read world director checkpoint {}: {error}",
            path.display()
        )
    })?;
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
    factory.install_checkpoint_bytes(&checkpoint.zone_factory_checkpoint)?;
    Ok(WorldDirectorRuntimeState {
        control_log,
        adapter: Mir2DirectorSimulationAdapter::restore(
            &checkpoint.simulation_checkpoint,
            trusted_director,
        )?,
        spawned_monsters_total: checkpoint.spawned_monsters_total,
        broadcast_messages_total: checkpoint.broadcast_messages_total,
        last_advance_at_ms: checkpoint.last_advance_at_ms,
        last_checkpoint_at_ms: checkpoint.last_checkpoint_at_ms,
        last_advance: checkpoint.last_advance,
    })
}

fn persist_runtime_checkpoint(
    path: &Path,
    state: &WorldDirectorRuntimeState,
    factory: &SharedInProcessZoneRuntimeFactory,
) -> Result<(), String> {
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
    let mut checkpoint = WorldDirectorRuntimeCheckpoint {
        version: DIRECTOR_RUNTIME_CHECKPOINT_VERSION,
        finalized: state.control_log.finalized(),
        simulation_checkpoint: state.adapter.checkpoint_bytes()?,
        zone_factory_checkpoint: factory.checkpoint_bytes()?,
        spawned_monsters_total: state.spawned_monsters_total,
        broadcast_messages_total: state.broadcast_messages_total,
        last_advance_at_ms: state.last_advance_at_ms,
        last_checkpoint_at_ms: state.last_checkpoint_at_ms,
        last_advance: state.last_advance.clone(),
        state_commitment: String::new(),
    };
    checkpoint.state_commitment = runtime_checkpoint_commitment(&checkpoint)?;
    let bytes = serde_json::to_vec_pretty(&checkpoint)
        .map_err(|error| format!("world director runtime checkpoint encode failed: {error}"))?;
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
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
                level: template.level,
                max_hp,
                hp: max_hp,
                experience: (u64::from(template.experience) * u64::from(reward_modifier_bps)
                    / BASIS_POINTS)
                    .min(u64::from(u32::MAX)) as u32,
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
        assert_eq!(service.status().unwrap().finalized_height, 1);

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
        assert_eq!(restored.status().unwrap().installed_command_count, 1);
        let incursion = restored.advance(now_ms + 5 * 60 * 1_000).unwrap();
        assert_eq!(incursion.spawned_monsters, 24);
        let status = restored.status().unwrap();
        assert_eq!(status.spawned_monsters_total, 24);
        assert_eq!(status.world_event_monsters_by_zone["map:D022"], 24);
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
}
