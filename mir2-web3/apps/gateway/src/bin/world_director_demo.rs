use std::env;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use mir2_gateway::{
    director_commands_from_finalized, CommonwareControlLog, DirectorPolicyState,
    DirectorPressureScores, DirectorProposal, EconomyTelemetrySnapshot,
    FinalizedDirectorSubmission, GuildTelemetrySnapshot, MapTelemetrySnapshot,
    Mir2DirectorSimulationAdapter, NodeSigningIdentity, SharedInProcessZoneRuntimeFactory,
    SignedDirectorCommand, WorldDirectorPolicy, WorldTelemetrySnapshot, ZoneDirectorExecutor,
    ZoneId, WORLD_DIRECTOR_SCHEMA,
};
use serde::Serialize;

const NOW_MS: u64 = 1_774_800_000_000;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DemoEvidence {
    schema: &'static str,
    scenario: &'static str,
    issued_at_ms: u64,
    live_elapsed_ms: u64,
    snapshot_id: String,
    pressure_scores: DirectorPressureScores,
    proposal_id: String,
    command_id: String,
    director_public_key: String,
    commonware_height: u64,
    commonware_committee: Vec<String>,
    commonware_signers: Vec<String>,
    finalized_block_file: Option<String>,
    zone_host_id: String,
    scheduled_stage_ids: Vec<String>,
    execution_state_commitment: String,
    command_signature_verified: bool,
    receipt_signature_verified: bool,
    idempotent_replay_verified: bool,
    simulation_spawned_monsters: usize,
    simulation_restart_recovery_verified: bool,
    wooma_vanguard_count: usize,
    awakened_boss_count: usize,
}

fn main() -> Result<(), String> {
    let finalized_block_file = env::var("MIR2_WORLD_DIRECTOR_FINALIZED_OUT")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let wall_now_ms = if finalized_block_file.is_some() {
        live_now_ms()
    } else {
        NOW_MS
    };
    let live_elapsed_ms = if finalized_block_file.is_some() {
        env::var("MIR2_WORLD_DIRECTOR_ELAPSED_MS")
            .ok()
            .map(|value| {
                value
                    .parse::<u64>()
                    .map_err(|_| "MIR2_WORLD_DIRECTOR_ELAPSED_MS must be an integer".to_string())
            })
            .transpose()?
            .unwrap_or_default()
    } else {
        0
    };
    if live_elapsed_ms > 30 * 60 * 1_000 {
        return Err("MIR2_WORLD_DIRECTOR_ELAPSED_MS must not exceed 30 minutes".to_string());
    }
    let now_ms = wall_now_ms.saturating_sub(live_elapsed_ms);
    let snapshot = demo_snapshot(now_ms);
    let scores = DirectorPressureScores::from_snapshot(&snapshot)?;
    let proposal = DirectorProposal::bichon_wooma_rule(&snapshot, &scores, now_ms)
        .ok_or_else(|| "demo pressure did not trigger the Bichon-Wooma template".to_string())?;
    let policy = WorldDirectorPolicy::mir2_default();
    let plan = policy.approve(
        &proposal,
        &snapshot,
        &scores,
        &DirectorPolicyState::default(),
        now_ms,
    )?;

    let director = NodeSigningIdentity::from_seed([71; 32]);
    let command = SignedDirectorCommand::issue(
        &plan,
        &snapshot,
        &director,
        live_elapsed_ms.saturating_add(5 * 60 * 1_000),
    )?;
    command.verify(director.public_key(), now_ms)?;

    let validator_identities = [
        NodeSigningIdentity::from_seed([81; 32]),
        NodeSigningIdentity::from_seed([82; 32]),
        NodeSigningIdentity::from_seed([83; 32]),
        NodeSigningIdentity::from_seed([84; 32]),
    ];
    let validators = validator_identities
        .iter()
        .map(|validator| validator.public_key().to_string())
        .collect::<Vec<_>>();
    let control = CommonwareControlLog::new(validators.clone())?;
    let block = control.propose(&validators[0], vec![command.control_envelope()?])?;
    control.vote(&validators[0], &block.digest)?;
    control.vote(&validators[1], &block.digest)?;
    let finalized = control
        .vote(&validators[2], &block.digest)?
        .ok_or_else(|| "demo command did not reach Commonware quorum".to_string())?;
    let submission = FinalizedDirectorSubmission::issue(finalized.clone(), &validator_identities)?;
    let commands = director_commands_from_finalized(&finalized, director.public_key(), now_ms)?;
    if let Some(path) = finalized_block_file.as_deref() {
        let bytes = serde_json::to_vec_pretty(&submission)
            .map_err(|error| format!("finalized submission encode failed: {error}"))?;
        fs::write(path, [&bytes[..], b"\n"].concat())
            .map_err(|error| format!("failed to write finalized block {path}: {error}"))?;
    }

    let zone_identity = NodeSigningIdentity::from_seed([72; 32]);
    let mut zone = ZoneDirectorExecutor::new(
        "dubhe-zone-host-hk-01",
        director.public_key(),
        zone_identity.clone(),
    )?;
    let receipt = zone.execute(&commands[0], finalized.block.height, now_ms)?;
    receipt.verify(zone_identity.public_key())?;
    let replay = zone.execute(&commands[0], finalized.block.height, now_ms + 1)?;
    let idempotent_replay_verified = receipt == replay;

    let factory = SharedInProcessZoneRuntimeFactory::new();
    let mut simulation = Mir2DirectorSimulationAdapter::new(director.public_key())?;
    simulation.install(commands[0].clone(), now_ms)?;
    simulation.advance(now_ms, &factory)?;
    let checkpoint = simulation.checkpoint_bytes()?;
    let mut restored = Mir2DirectorSimulationAdapter::restore(&checkpoint, director.public_key())?;
    let incursion = restored.advance(now_ms + 5 * 60 * 1_000, &factory)?;
    let finale = restored.advance(now_ms + 20 * 60 * 1_000, &factory)?;
    let simulation_restart_recovery_verified =
        restored.checkpoint_bytes()? != checkpoint && incursion.spawned_monsters == 24;
    let wooma_vanguard_count =
        factory.world_event_monster_count(&ZoneId::new("map:D022"), "D022")?;
    let awakened_boss_count =
        factory.world_event_monster_count(&ZoneId::new("map:D024"), "D024")?;

    let evidence = DemoEvidence {
        schema: WORLD_DIRECTOR_SCHEMA,
        scenario: "mir2-bichon-wooma-awakening",
        issued_at_ms: now_ms,
        live_elapsed_ms,
        snapshot_id: snapshot.snapshot_id,
        pressure_scores: scores,
        proposal_id: proposal.proposal_id,
        command_id: command.payload.command_id,
        director_public_key: director.public_key().to_string(),
        commonware_height: finalized.block.height,
        commonware_committee: validators,
        commonware_signers: finalized.signers.iter().cloned().collect(),
        finalized_block_file,
        zone_host_id: receipt.zone_host_id,
        scheduled_stage_ids: receipt
            .applied_stages
            .iter()
            .map(|stage| stage.stage_id.clone())
            .collect(),
        execution_state_commitment: receipt.state_commitment,
        command_signature_verified: true,
        receipt_signature_verified: true,
        idempotent_replay_verified,
        simulation_spawned_monsters: incursion.spawned_monsters + finale.spawned_monsters,
        simulation_restart_recovery_verified,
        wooma_vanguard_count,
        awakened_boss_count,
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&evidence)
            .map_err(|error| format!("demo evidence encode failed: {error}"))?
    );
    Ok(())
}

fn demo_snapshot(now_ms: u64) -> WorldTelemetrySnapshot {
    WorldTelemetrySnapshot {
        schema: WORLD_DIRECTOR_SCHEMA.to_string(),
        snapshot_id: "world-hk-15m-000001".to_string(),
        game_id: "mir2".to_string(),
        region_id: "asia-hk".to_string(),
        observed_at_ms: now_ms,
        window_ms: 15 * 60 * 1_000,
        maps: vec![
            map("map:0", 80, 18, 20, 5, 8_000, 8, 20, 50),
            map("map:D022", 20, 26, 0, 0, 4_000, 4, 15, 15),
            map("map:D023", 12, 29, 0, 0, 3_000, 6, 12, 8),
            map("map:D024", 8, 31, 0, 0, 1_000, 12, 30, 2),
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

fn live_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(NOW_MS)
}

#[allow(clippy::too_many_arguments)]
fn map(
    zone_id: &str,
    active_players: u32,
    median_level: u16,
    new_player_count: u32,
    returning_player_count: u32,
    monster_kills: u64,
    boss_kills: u64,
    player_deaths: u64,
    completed_quests: u64,
) -> MapTelemetrySnapshot {
    MapTelemetrySnapshot {
        zone_id: zone_id.to_string(),
        active_players,
        median_level,
        new_player_count,
        returning_player_count,
        monster_kills,
        boss_kills,
        player_deaths,
        completed_quests,
    }
}
