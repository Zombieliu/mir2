use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mir2_game_data::{crystal_respawn_manifest_ref, CrystalRespawnMap};
use mir2_gateway::economy::{EconomyBalanceKey, EconomyReconciliationReport, PostgresEconomyStore};
use mir2_gateway::zone_lease::{
    default_zone_owner_lease_authority_from_env, PostgresZoneOwnerLeaseAuthority,
};
use mir2_gateway::{
    GatewayConfig, GatewaySession, RegionalProfile, TcpZoneOwnerRpcTransport, ZoneId,
    ZoneRpcLimits, ZoneTopology,
};
use mir2_protocol::{ClientPacket, MirClass, MirDirection, MirGender, ServerPacket};
use mir2_simulation::{WorldCommand, WorldEntityKind};
use postgres::{Client, NoTls};
use serde::Serialize;

const DEFAULT_OUTPUT: &str = "docs/generated/regional/gate18-load.json";
const ACTIVE_OWNER: &str = "gate18-active";
const STANDBY_OWNER: &str = "gate18-standby";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
enum WorkloadRole {
    Movement,
    Combat,
    Social,
    Economy,
    Idle,
}

#[derive(Debug)]
struct LoadPlayer {
    session: GatewaySession,
    account_id: String,
    character_index: i32,
    role: WorkloadRole,
    command_ordinal: u64,
    combat_target: u32,
    economy_drop: Option<(u32, i32, i32)>,
    economy_transitions: Vec<EconomyTransitionEvidence>,
}

#[derive(Debug, Default)]
struct SharedMetrics {
    attempted: u64,
    completed: u64,
    failed: u64,
    latencies_ms: Vec<f64>,
    completed_by_role: BTreeMap<WorkloadRole, u64>,
    failures: BTreeMap<String, u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NumericSummary {
    count: usize,
    minimum: Option<f64>,
    mean: Option<f64>,
    p50: Option<f64>,
    p95: Option<f64>,
    p99: Option<f64>,
    maximum: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PromotionEvidence {
    zone_id: String,
    zone_session_count: usize,
    active_owner: String,
    active_generation: u64,
    standby_owner: String,
    standby_generation: u64,
    quiesced_at_ms: u64,
    readiness_id: String,
    readiness_lag_ms: u64,
    base_sequence: u64,
    promoted_sequence: u64,
    session_refresh_count: usize,
    post_promotion_probe_count: usize,
    wall_ms: f64,
    success: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourceEvidence {
    available_parallelism: usize,
    cgroup_cpu_max: Option<String>,
    cgroup_memory_max: Option<String>,
    cgroup_memory_current: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EconomyMismatchEvidence {
    account_id: String,
    character_index: i32,
    command_ordinal: u64,
    runtime_gold: i64,
    ledger_gold: Option<i64>,
    ledger_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct EconomyTransitionEvidence {
    account_id: String,
    character_index: i32,
    command_ordinal: u64,
    operation: String,
    before_gold: u32,
    after_gold: u32,
    object_id: Option<u32>,
    object_x: Option<i32>,
    object_y: Option<i32>,
    semantic_success: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Gate18LoadEvidence {
    schema_version: u32,
    generated_at_ms: u64,
    run_id: String,
    profile_id: String,
    profile_exact: bool,
    requested_players: usize,
    connected_players: usize,
    distinct_accounts: usize,
    distinct_characters: usize,
    profile_catalog_maps: usize,
    runtime_manifest_maps: usize,
    active_zone_count: usize,
    requested_active_duration_seconds: u64,
    measured_active_duration_ms: u128,
    promotion_pause_excluded_from_active_duration: bool,
    worker_threads: usize,
    roles: BTreeMap<WorkloadRole, usize>,
    attempted_commands: u64,
    completed_commands: u64,
    failed_commands: u64,
    error_rate: f64,
    expected_commands_by_role: BTreeMap<WorkloadRole, u64>,
    completed_by_role: BTreeMap<WorkloadRole, u64>,
    workload_command_coverage: f64,
    failure_reasons: BTreeMap<String, u64>,
    latency_ms: NumericSummary,
    promotion: PromotionEvidence,
    economy_duplicate_count: i64,
    economy_runtime_ledger_mismatch_count: usize,
    economy_mismatches: Vec<EconomyMismatchEvidence>,
    economy_transitions: Vec<EconomyTransitionEvidence>,
    economy_reconciliation: EconomyReconciliationReport,
    resources: ResourceEvidence,
    assertions: BTreeMap<String, bool>,
    success: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let profile = RegionalProfile::reference()?;
    profile.require_reference_contract()?;
    let requested_players = env_usize(
        "MIR2_GATE18_LOAD_PLAYERS",
        profile.stages.gate18.concurrent_players,
    );
    let duration_seconds = env_u64(
        "MIR2_GATE18_LOAD_DURATION_SECONDS",
        profile.stages.gate18.duration_seconds,
    );
    let worker_threads = env_usize("MIR2_GATE18_LOAD_WORKERS", 256)
        .max(1)
        .min(requested_players.max(1));
    let profile_exact = requested_players == profile.stages.gate18.concurrent_players
        && duration_seconds == profile.stages.gate18.duration_seconds;
    let allow_dev_profile = env_bool("MIR2_GATE18_ALLOW_DEV_PROFILE");
    let database_url = env::var("MIR2_ECONOMY_DATABASE_URL")
        .map_err(|_| "MIR2_ECONOMY_DATABASE_URL is required")?;
    let output = PathBuf::from(
        env::var("MIR2_GATE18_LOAD_OUT").unwrap_or_else(|_| DEFAULT_OUTPUT.to_string()),
    );
    let generated_at_ms = now_ms();
    let run_id = format!("gate18-load-{generated_at_ms}");

    let store = PostgresEconomyStore::new(database_url.clone());
    store.ensure_migrated()?;
    let topology = ZoneTopology::from_env()?;
    let registry = topology.zone_registry(default_zone_owner_lease_authority_from_env());
    let config = GatewayConfig::default().with_crystal_world_runtime();
    let roles = role_counts(requested_players, &profile);
    let manifest = crystal_respawn_manifest_ref();
    let active_maps = manifest
        .maps
        .iter()
        .filter(|map| !map.respawns.is_empty())
        .take(profile.active_maps)
        .collect::<Vec<_>>();
    if active_maps.len() != profile.active_maps {
        return Err(format!(
            "Crystal manifest only supplied {} populated maps; Regional requires {}",
            active_maps.len(),
            profile.active_maps
        )
        .into());
    }
    let mut players = Vec::with_capacity(requested_players);
    for player_index in 0..requested_players {
        let role = role_for_index(player_index, &roles);
        let account_id = format!(
            "g18l{:09}{:04}",
            generated_at_ms % 1_000_000_000,
            player_index
        );
        let character_name = format!("L{:06}{:03}", generated_at_ms % 1_000_000, player_index);
        let (mut session, character_index) =
            start_session(&registry, config.clone(), &account_id, &character_name)?;
        let map = active_maps[player_index % active_maps.len()];
        let (target_map, position) = if role == WorkloadRole::Economy {
            let economy_index = player_index.saturating_sub(requested_players * 80 / 100);
            (
                "0",
                (
                    330 + i32::try_from(economy_index % 5).unwrap_or_default() * 3,
                    270 + i32::try_from(economy_index / 5).unwrap_or_default() * 3,
                ),
            )
        } else {
            (
                map.map_file_name.as_str(),
                map_fixture_position(map, player_index / active_maps.len()),
            )
        };
        session.transfer_map(&format!(
            "crystal:{target_map}:{}:{}",
            position.0, position.1
        ));
        let expected_zone = format!("map:{target_map}");
        if session.zone_id().as_str() != expected_zone {
            return Err(format!(
                "player {player_index} routed to {}, expected {expected_zone}",
                session.zone_id()
            )
            .into());
        }
        if role == WorkloadRole::Economy {
            // Seed the deterministic opening balance on the authoritative
            // hosted session before the measured production workload.
            apply_gold_fixture(&mut session, 100)?;
        }
        let combat_target = if role == WorkloadRole::Combat {
            session
                .world_snapshot()
                .entities
                .into_iter()
                .find(|entity| entity.kind == WorldEntityKind::Monster && !entity.dead)
                .map(|entity| entity.object_id)
                .unwrap_or_default()
        } else {
            0
        };
        players.push(LoadPlayer {
            session,
            account_id,
            character_index,
            role,
            command_ordinal: 0,
            combat_target,
            economy_drop: None,
            economy_transitions: Vec::new(),
        });
        if (player_index + 1) % 50 == 0 || player_index + 1 == requested_players {
            eprintln!(
                "Gate 18 load: connected {}/{} players",
                player_index + 1,
                requested_players
            );
        }
    }

    let distinct_accounts = players
        .iter()
        .map(|player| player.account_id.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let distinct_characters = players
        .iter()
        .map(|player| (player.account_id.as_str(), player.character_index))
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let active_zone_count = players
        .iter()
        .map(|player| player.session.zone_id().as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let metrics = Arc::new(Mutex::new(SharedMetrics::default()));
    let first_phase_seconds = duration_seconds / 2;
    let second_phase_seconds = duration_seconds.saturating_sub(first_phase_seconds);
    let active_started = Instant::now();
    run_phase(
        &mut players,
        Duration::from_secs(first_phase_seconds),
        worker_threads,
        &profile,
        Arc::clone(&metrics),
    );
    let first_phase_elapsed = active_started.elapsed();
    let promotion = promote_zone(&mut players, &database_url)?;
    let second_phase_started = Instant::now();
    run_phase(
        &mut players,
        Duration::from_secs(second_phase_seconds),
        worker_threads,
        &profile,
        Arc::clone(&metrics),
    );
    let measured_active_duration_ms =
        first_phase_elapsed.as_millis() + second_phase_started.elapsed().as_millis();

    let economy_mismatches = players
        .iter()
        .filter(|player| player.role == WorkloadRole::Economy)
        .filter_map(|player| {
            let runtime_gold = i64::from(player.session.world_snapshot().gold);
            let key = EconomyBalanceKey::gold(&player.account_id, player.character_index);
            match store.balance(&key) {
                Ok(ledger_gold) if ledger_gold == runtime_gold => None,
                Ok(ledger_gold) => Some(EconomyMismatchEvidence {
                    account_id: player.account_id.clone(),
                    character_index: player.character_index,
                    command_ordinal: player.command_ordinal,
                    runtime_gold,
                    ledger_gold: Some(ledger_gold),
                    ledger_error: None,
                }),
                Err(error) => Some(EconomyMismatchEvidence {
                    account_id: player.account_id.clone(),
                    character_index: player.character_index,
                    command_ordinal: player.command_ordinal,
                    runtime_gold,
                    ledger_gold: None,
                    ledger_error: Some(error),
                }),
            }
        })
        .collect::<Vec<_>>();
    let economy_runtime_ledger_mismatch_count = economy_mismatches.len();
    let economy_transitions = players
        .iter()
        .flat_map(|player| player.economy_transitions.iter().cloned())
        .collect::<Vec<_>>();
    let economy_duplicate_count = duplicate_economy_count(&database_url)?;
    let economy_reconciliation = store.reconcile(now_ms())?;
    let mut metrics = Arc::try_unwrap(metrics)
        .map_err(|_| "Gate 18 metrics still have live references")?
        .into_inner()
        .map_err(|_| "Gate 18 metrics mutex poisoned")?;
    let latency_ms = summarize(&mut metrics.latencies_ms);
    let error_rate = if metrics.attempted == 0 {
        1.0
    } else {
        metrics.failed as f64 / metrics.attempted as f64
    };
    let expected_commands_by_role = expected_commands_by_role(&roles, duration_seconds, &profile);
    let expected_commands = expected_commands_by_role.values().sum::<u64>();
    let workload_command_coverage = if expected_commands == 0 {
        0.0
    } else {
        metrics.completed as f64 / expected_commands as f64
    };
    let assertions = BTreeMap::from([
        (
            "profileContractAccepted".to_string(),
            (profile_exact
                && requested_players == 500
                && duration_seconds == 1_800
                && profile.profile_id == "mir2-regional-v1")
                || allow_dev_profile,
        ),
        (
            "allPlayersConnected".to_string(),
            players.len() == requested_players,
        ),
        (
            "allAccountsAndCharactersDistinct".to_string(),
            distinct_accounts == requested_players && distinct_characters == requested_players,
        ),
        (
            "workloadMixMatchesProfile".to_string(),
            role_mix_matches(&roles, requested_players, &profile),
        ),
        (
            "activeZoneCountMatchesProfile".to_string(),
            active_zone_count == profile.active_maps.min(requested_players),
        ),
        (
            "allWorkloadRolesExecuted".to_string(),
            metrics.completed_by_role.len() == 5
                && metrics.completed_by_role.values().all(|count| *count > 0),
        ),
        (
            "workloadCommandRateSustained".to_string(),
            workload_command_coverage >= 0.95
                && expected_commands_by_role.iter().all(|(role, expected)| {
                    metrics
                        .completed_by_role
                        .get(role)
                        .copied()
                        .unwrap_or_default() as f64
                        >= *expected as f64 * 0.95
                }),
        ),
        (
            "errorRateWithinGate18Slo".to_string(),
            error_rate <= profile.stages.gate18.maximum_error_rate,
        ),
        ("safeZonePromotionCompleted".to_string(), promotion.success),
        (
            "economyHasNoDuplicateResult".to_string(),
            economy_duplicate_count == 0,
        ),
        (
            "economyRuntimeMatchesLedger".to_string(),
            economy_runtime_ledger_mismatch_count == 0,
        ),
        (
            "economyReconciliationHealthy".to_string(),
            economy_reconciliation.healthy,
        ),
    ]);
    let success = assertions.values().all(|passed| *passed);
    let evidence = Gate18LoadEvidence {
        schema_version: 1,
        generated_at_ms,
        run_id,
        profile_id: profile.profile_id.clone(),
        profile_exact,
        requested_players,
        connected_players: players.len(),
        distinct_accounts,
        distinct_characters,
        profile_catalog_maps: profile.catalog_maps,
        runtime_manifest_maps: manifest.maps.len(),
        active_zone_count,
        requested_active_duration_seconds: duration_seconds,
        measured_active_duration_ms,
        promotion_pause_excluded_from_active_duration: true,
        worker_threads,
        roles,
        attempted_commands: metrics.attempted,
        completed_commands: metrics.completed,
        failed_commands: metrics.failed,
        error_rate,
        expected_commands_by_role,
        completed_by_role: metrics.completed_by_role,
        workload_command_coverage,
        failure_reasons: metrics.failures,
        latency_ms,
        promotion,
        economy_duplicate_count,
        economy_runtime_ledger_mismatch_count,
        economy_mismatches,
        economy_transitions,
        economy_reconciliation,
        resources: resource_evidence(),
        assertions,
        success,
    };
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        &output,
        format!("{}\n", serde_json::to_string_pretty(&evidence)?),
    )?;
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    if !success {
        return Err("Gate 18 mixed load acceptance failed".into());
    }
    Ok(())
}

fn role_counts(players: usize, profile: &RegionalProfile) -> BTreeMap<WorkloadRole, usize> {
    let movement = players * usize::from(profile.workload.movement_players_percent) / 100;
    let combat = players * usize::from(profile.workload.combat_players_percent) / 100;
    let social_economy =
        players * usize::from(profile.workload.social_economy_players_percent) / 100;
    let social = social_economy / 2;
    let economy = social_economy - social;
    let assigned = movement + combat + social + economy;
    BTreeMap::from([
        (WorkloadRole::Movement, movement),
        (WorkloadRole::Combat, combat),
        (WorkloadRole::Social, social),
        (WorkloadRole::Economy, economy),
        (WorkloadRole::Idle, players.saturating_sub(assigned)),
    ])
}

fn role_for_index(index: usize, counts: &BTreeMap<WorkloadRole, usize>) -> WorkloadRole {
    let mut cursor = 0;
    for role in [
        WorkloadRole::Movement,
        WorkloadRole::Combat,
        WorkloadRole::Social,
        WorkloadRole::Economy,
        WorkloadRole::Idle,
    ] {
        cursor += counts.get(&role).copied().unwrap_or_default();
        if index < cursor {
            return role;
        }
    }
    WorkloadRole::Idle
}

fn map_fixture_position(map: &CrystalRespawnMap, local_player_index: usize) -> (i32, i32) {
    let origin = map
        .respawns
        .first()
        .map(|respawn| respawn.location.clone())
        .or_else(|| {
            map.safe_zones
                .first()
                .map(|safe_zone| safe_zone.location.clone())
        })
        .or_else(|| {
            map.movements
                .first()
                .map(|movement| movement.source.clone())
        })
        .unwrap_or(mir2_protocol::Point { x: 100, y: 100 });
    (
        origin
            .x
            .saturating_add(i32::try_from(local_player_index).unwrap_or_default() * 2),
        origin.y,
    )
}

fn role_mix_matches(
    counts: &BTreeMap<WorkloadRole, usize>,
    players: usize,
    profile: &RegionalProfile,
) -> bool {
    counts
        .get(&WorkloadRole::Movement)
        .copied()
        .unwrap_or_default()
        * 100
        == players * usize::from(profile.workload.movement_players_percent)
        && counts
            .get(&WorkloadRole::Combat)
            .copied()
            .unwrap_or_default()
            * 100
            == players * usize::from(profile.workload.combat_players_percent)
        && (counts
            .get(&WorkloadRole::Social)
            .copied()
            .unwrap_or_default()
            + counts
                .get(&WorkloadRole::Economy)
                .copied()
                .unwrap_or_default())
            * 100
            == players * usize::from(profile.workload.social_economy_players_percent)
        && counts.get(&WorkloadRole::Idle).copied().unwrap_or_default() * 100
            == players * usize::from(profile.workload.idle_players_percent)
}

fn expected_commands_by_role(
    counts: &BTreeMap<WorkloadRole, usize>,
    duration_seconds: u64,
    profile: &RegionalProfile,
) -> BTreeMap<WorkloadRole, u64> {
    let count = |role| counts.get(&role).copied().unwrap_or_default() as u64;
    BTreeMap::from([
        (
            WorkloadRole::Movement,
            count(WorkloadRole::Movement)
                * u64::from(profile.workload.movement_commands_per_second)
                * duration_seconds,
        ),
        (
            WorkloadRole::Combat,
            count(WorkloadRole::Combat)
                * u64::from(profile.workload.combat_commands_per_second)
                * duration_seconds,
        ),
        (
            WorkloadRole::Social,
            count(WorkloadRole::Social)
                * u64::from(profile.workload.social_economy_transactions_per_minute)
                * duration_seconds
                / 60,
        ),
        (
            WorkloadRole::Economy,
            count(WorkloadRole::Economy)
                * u64::from(profile.workload.social_economy_transactions_per_minute)
                * duration_seconds
                / 60,
        ),
        (
            WorkloadRole::Idle,
            count(WorkloadRole::Idle)
                * duration_seconds.div_ceil(profile.workload.idle_keep_alive_seconds),
        ),
    ])
}

fn run_phase(
    players: &mut [LoadPlayer],
    duration: Duration,
    worker_threads: usize,
    profile: &RegionalProfile,
    metrics: Arc<Mutex<SharedMetrics>>,
) {
    if duration.is_zero() || players.is_empty() {
        return;
    }
    let deadline = Instant::now() + duration;
    let phase_started = Instant::now();
    let chunk_size = players.len().div_ceil(worker_threads);
    thread::scope(|scope| {
        for (chunk_index, chunk) in players.chunks_mut(chunk_size).enumerate() {
            let metrics = Arc::clone(&metrics);
            thread::Builder::new()
                .name(format!("gate18-load-{chunk_index}"))
                .stack_size(256 * 1024)
                .spawn_scoped(scope, move || {
                    let mut due = chunk
                        .iter()
                        .enumerate()
                        .map(|(index, player)| {
                            let global_index = chunk_index * chunk_size + index;
                            let phase_slot =
                                (global_index as u64).wrapping_mul(2_654_435_761) % 1_024;
                            let fraction = phase_slot as f64 / 1_024.0;
                            phase_started + role_interval(player.role, profile).mul_f64(fraction)
                        })
                        .collect::<Vec<_>>();
                    while Instant::now() < deadline {
                        let mut next_due = deadline;
                        for (index, player) in chunk.iter_mut().enumerate() {
                            let now = Instant::now();
                            if now >= due[index] {
                                execute_player_command(player, &metrics);
                                due[index] += role_interval(player.role, profile);
                                if due[index] <= now {
                                    due[index] = now + role_interval(player.role, profile);
                                }
                            }
                            next_due = next_due.min(due[index]);
                        }
                        let now = Instant::now();
                        if next_due > now {
                            thread::sleep((next_due - now).min(Duration::from_millis(5)));
                        }
                    }
                })
                .expect("Gate 18 load worker should spawn");
        }
    });
}

fn role_interval(role: WorkloadRole, profile: &RegionalProfile) -> Duration {
    match role {
        WorkloadRole::Movement => {
            Duration::from_secs_f64(1.0 / f64::from(profile.workload.movement_commands_per_second))
        }
        WorkloadRole::Combat => {
            Duration::from_secs_f64(1.0 / f64::from(profile.workload.combat_commands_per_second))
        }
        WorkloadRole::Social | WorkloadRole::Economy => Duration::from_secs_f64(
            60.0 / f64::from(profile.workload.social_economy_transactions_per_minute),
        ),
        WorkloadRole::Idle => Duration::from_secs(profile.workload.idle_keep_alive_seconds),
    }
}

fn execute_player_command(player: &mut LoadPlayer, metrics: &Arc<Mutex<SharedMetrics>>) {
    if player.role == WorkloadRole::Economy
        && player.command_ordinal == 0
        && player.session.world_snapshot().gold != 100
    {
        // A busy shared hotspot may publish an older presence snapshot while
        // the remaining sessions are still connecting. Reassert the fixture
        // on the authoritative hosted session immediately before its first
        // fenced production economy transaction.
        if let Err(error) = apply_gold_fixture(&mut player.session, 100) {
            let mut metrics = metrics.lock().expect("Gate 18 metrics mutex poisoned");
            metrics.attempted += 1;
            metrics.failed += 1;
            *metrics
                .failures
                .entry(format!("economy_fixture_failed:{error}"))
                .or_default() += 1;
            return;
        }
    }
    let economy_before_gold =
        (player.role == WorkloadRole::Economy).then(|| player.session.world_snapshot().gold);
    let economy_is_drop = player.role == WorkloadRole::Economy && player.command_ordinal % 2 == 0;
    let command = match player.role {
        WorkloadRole::Movement => {
            let direction = if player.command_ordinal % 2 == 0 {
                MirDirection::Right
            } else {
                MirDirection::Left
            };
            WorldCommand::ClientPacket(ClientPacket::Walk { direction })
        }
        WorkloadRole::Combat => WorldCommand::Attack {
            object_id: player.combat_target,
        },
        WorkloadRole::Social => WorldCommand::ClientPacket(ClientPacket::Chat {
            message: format!("regional-{}", player.command_ordinal),
            linked_items: Vec::new(),
        }),
        WorkloadRole::Economy => {
            if economy_is_drop {
                WorldCommand::ClientPacket(ClientPacket::DropGold { amount: 1 })
            } else if let Some((object_id, x, y)) = player.economy_drop {
                let _ = player.session.move_to(x, y, false);
                WorldCommand::PickUp { object_id }
            } else {
                WorldCommand::ClientPacket(ClientPacket::PickUp)
            }
        }
        WorkloadRole::Idle => WorldCommand::ClientPacket(ClientPacket::KeepAlive {
            time: now_ms().try_into().unwrap_or(i64::MAX),
        }),
    };
    player.command_ordinal += 1;
    let started = Instant::now();
    let result = player
        .session
        .execute_production_player_command(true, command);
    let latency = started.elapsed().as_secs_f64() * 1_000.0;
    let result = result.and_then(|execution| {
        let Some(before_gold) = economy_before_gold else {
            return Ok(execution);
        };
        let after_snapshot = player.session.world_snapshot();
        let after_gold = after_snapshot.gold;
        let expected_gold = if economy_is_drop {
            before_gold.saturating_sub(1)
        } else {
            before_gold.saturating_add(1)
        };
        let receipt_drop = economy_is_drop
            .then(|| {
                execution
                    .packets
                    .iter()
                    .rev()
                    .find_map(|packet| match packet {
                        ServerPacket::ObjectGold { info } if info.gold == 1 => {
                            Some((info.object_id, info.location.x, info.location.y))
                        }
                        _ => None,
                    })
            })
            .flatten();
        let object = if economy_is_drop {
            receipt_drop
        } else {
            player.economy_drop
        };
        let semantic_success =
            after_gold == expected_gold && (!economy_is_drop || receipt_drop.is_some());
        player.economy_transitions.push(EconomyTransitionEvidence {
            account_id: player.account_id.clone(),
            character_index: player.character_index,
            command_ordinal: player.command_ordinal.saturating_sub(1),
            operation: if economy_is_drop { "drop" } else { "pickup" }.to_string(),
            before_gold,
            after_gold,
            object_id: object.map(|(object_id, _, _)| object_id),
            object_x: object.map(|(_, x, _)| x),
            object_y: object.map(|(_, _, y)| y),
            semantic_success,
        });
        if semantic_success {
            player.economy_drop = if economy_is_drop { receipt_drop } else { None };
            Ok(execution)
        } else if economy_is_drop && receipt_drop.is_none() {
            Err("economy_semantic_mismatch:drop_missing_object".to_string())
        } else {
            Err(format!(
                "economy_semantic_mismatch:{}:{}:{}",
                if economy_is_drop { "drop" } else { "pickup" },
                before_gold,
                after_gold
            ))
        }
    });
    let mut metrics = metrics.lock().expect("Gate 18 metrics mutex poisoned");
    metrics.attempted += 1;
    match result {
        Ok(_) => {
            metrics.completed += 1;
            metrics.latencies_ms.push(latency);
            *metrics.completed_by_role.entry(player.role).or_default() += 1;
        }
        Err(error) => {
            metrics.failed += 1;
            *metrics.failures.entry(error).or_default() += 1;
        }
    }
}

fn promote_zone(
    players: &mut [LoadPlayer],
    database_url: &str,
) -> Result<PromotionEvidence, Box<dyn std::error::Error>> {
    let started = Instant::now();
    let zone_id = ZoneId::new("map:0");
    let addresses = env::var("MIR2_ZONE_HOST_ADDRS")
        .map_err(|_| "MIR2_ZONE_HOST_ADDRS with active and standby is required")?;
    let mut endpoints = addresses.split(',').map(str::trim);
    let active_address = endpoints.next().ok_or("active Zone endpoint missing")?;
    let standby_address = endpoints.next().ok_or("standby Zone endpoint missing")?;
    let token = env::var("MIR2_ZONE_HOST_TOKEN")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let limits = ZoneRpcLimits::from_env();
    let active = TcpZoneOwnerRpcTransport::with_options(
        active_address,
        zone_id.clone(),
        "gate18-promotion-active",
        token.clone(),
        limits.clone(),
    );
    let standby = TcpZoneOwnerRpcTransport::with_options(
        standby_address,
        zone_id.clone(),
        "gate18-promotion-standby",
        token,
        limits,
    );
    let active_lease = players
        .first()
        .ok_or("promotion requires at least one player")?
        .session
        .zone_owner_lease()
        .clone();
    let promotion_zone_session_count = players
        .iter()
        .filter(|player| player.session.zone_id() == &zone_id)
        .count();
    let quiesce = active.quiesce_for_promotion(&active_lease)?;
    let base = active.export_base_snapshot()?;
    standby.install_base_snapshot(&base)?;
    let active_head = active.replication_head()?;
    let readiness = standby.assess_promotion_readiness(active_head.clone(), now_ms(), 500)?;
    if !readiness.ready {
        return Err(format!("standby was not promotion-ready: {readiness:?}").into());
    }
    let readiness_id = readiness
        .readiness_id
        .clone()
        .ok_or("ready standby returned no readiness id")?;
    let active_authority =
        PostgresZoneOwnerLeaseAuthority::new(database_url, ACTIVE_OWNER, lease_ttl_ms());
    let promoted_lease = active_authority.handoff_at(&active_lease, STANDBY_OWNER, now_ms())?;
    let receipt = standby.promote_replica(readiness_id.clone(), &promoted_lease)?;
    let mut session_refresh_count = 0;
    for player in players.iter_mut() {
        if player.session.zone_id() != &zone_id {
            continue;
        }
        let refreshed = player.session.refresh_zone_owner_lease()?;
        if refreshed.owner_id() != STANDBY_OWNER
            || refreshed.fencing_token() != promoted_lease.fencing_token()
        {
            return Err("player adopted the wrong post-promotion fence".into());
        }
        session_refresh_count += 1;
    }
    let mut post_promotion_probe_count = 0;
    for player in players.iter_mut() {
        player.session.execute_production_player_command(
            true,
            WorldCommand::ClientPacket(ClientPacket::KeepAlive {
                time: now_ms().try_into().unwrap_or(i64::MAX),
            }),
        )?;
        post_promotion_probe_count += 1;
    }
    Ok(PromotionEvidence {
        zone_id: zone_id.as_str().to_string(),
        zone_session_count: promotion_zone_session_count,
        active_owner: active_lease.owner_id().to_string(),
        active_generation: active_lease.fencing_token(),
        standby_owner: promoted_lease.owner_id().to_string(),
        standby_generation: promoted_lease.fencing_token(),
        quiesced_at_ms: quiesce.quiesced_at_ms,
        readiness_id,
        readiness_lag_ms: readiness.observed_lag_ms,
        base_sequence: base.base_sequence,
        promoted_sequence: receipt.head.next_sequence,
        session_refresh_count,
        post_promotion_probe_count,
        wall_ms: started.elapsed().as_secs_f64() * 1_000.0,
        success: active_lease.owner_id() == ACTIVE_OWNER
            && promoted_lease.owner_id() == STANDBY_OWNER
            && promoted_lease.fencing_token() > active_lease.fencing_token()
            && promotion_zone_session_count > 0
            && session_refresh_count == promotion_zone_session_count
            && post_promotion_probe_count == players.len(),
    })
}

fn start_session(
    registry: &mir2_gateway::ZoneRegistry,
    config: GatewayConfig,
    account_id: &str,
    character_name: &str,
) -> Result<(GatewaySession, i32), Box<dyn std::error::Error>> {
    let mut session = GatewaySession::new_with_zone_registry(config, registry);
    if session.on_connect().is_empty() {
        return Err("remote Zone Host returned no connect packets".into());
    }
    session.handle_packet(ClientPacket::NewAccount {
        account_id: account_id.to_string(),
        password: account_id.to_string(),
        birth_date_binary: 0,
        user_name: String::new(),
        secret_question: String::new(),
        secret_answer: String::new(),
        email_address: String::new(),
    });
    session.handle_packet(ClientPacket::Login {
        account_id: account_id.to_string(),
        password: account_id.to_string(),
    });
    let character_index = session
        .handle_packet(ClientPacket::NewCharacter {
            name: character_name.to_string(),
            gender: MirGender::Male,
            class: MirClass::Warrior,
        })
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
            _ => None,
        })
        .ok_or("remote character creation returned no index")?;
    let packets = session.handle_packet(ClientPacket::StartGame { character_index });
    if !packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. }))
    {
        return Err("remote character did not enter the world".into());
    }
    Ok((session, character_index))
}

fn apply_gold_fixture(
    session: &mut GatewaySession,
    gold: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let snapshot = session.world_snapshot();
    let identity = session
        .active_identity()
        .ok_or("gold fixture requires an active identity")?;
    let player = snapshot
        .entities
        .iter()
        .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
        .ok_or("gold fixture requires an in-world player")?;
    let state = serde_json::json!({
        "character": {
            "name": identity.character_name,
            "level": 1,
            "class": "Warrior",
            "gender": "Male"
        },
        "mapFileName": snapshot.map_file_name.unwrap_or_else(|| "0".to_string()),
        "mapTitle": snapshot.map_title.unwrap_or_else(|| "BichonProvince".to_string()),
        "position": { "x": player.x, "y": player.y },
        "direction": player.direction,
        "hp": 100,
        "maxHp": 100,
        "mp": 100,
        "maxMp": 100,
        "experience": 0,
        "maxExperience": 100,
        "gold": gold,
        "credit": 0,
        "inventoryItemsJson": [],
        "beltItemsJson": [],
        "storageItemsJson": [],
        "equipmentItemsJson": []
    });
    session.stage5_command("qa.applyNativeState", vec![state.to_string()]);
    if session.world_snapshot().gold != gold {
        return Err("gold fixture was not applied".into());
    }
    Ok(())
}

fn duplicate_economy_count(database_url: &str) -> Result<i64, Box<dyn std::error::Error>> {
    let mut client = Client::connect(database_url, NoTls)?;
    let count = client
        .query_one(
            "SELECT COALESCE(SUM(duplicates), 0)::bigint AS count
             FROM (
               SELECT COUNT(*) - 1 AS duplicates
               FROM game_economy_transactions
               GROUP BY idempotency_key
               HAVING COUNT(*) > 1
             ) duplicate_groups",
            &[],
        )?
        .get("count");
    Ok(count)
}

fn summarize(values: &mut [f64]) -> NumericSummary {
    values.sort_by(f64::total_cmp);
    let percentile = |fraction: f64| {
        if values.is_empty() {
            None
        } else {
            let index = ((values.len() - 1) as f64 * fraction).round() as usize;
            values.get(index).copied()
        }
    };
    NumericSummary {
        count: values.len(),
        minimum: values.first().copied(),
        mean: (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64),
        p50: percentile(0.50),
        p95: percentile(0.95),
        p99: percentile(0.99),
        maximum: values.last().copied(),
    }
}

fn resource_evidence() -> ResourceEvidence {
    ResourceEvidence {
        available_parallelism: thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1),
        cgroup_cpu_max: read_trimmed("/sys/fs/cgroup/cpu.max"),
        cgroup_memory_max: read_trimmed("/sys/fs/cgroup/memory.max"),
        cgroup_memory_current: read_trimmed("/sys/fs/cgroup/memory.current"),
    }
}

fn read_trimmed(path: &str) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn lease_ttl_ms() -> u64 {
    env_u64("MIR2_GATEWAY_ZONE_LEASE_TTL_MS", 30_000)
}

fn env_usize(name: &str, default: usize) -> usize {
    env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn env_u64(name: &str, default: u64) -> u64 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn env_bool(name: &str) -> bool {
    env::var(name)
        .ok()
        .is_some_and(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "YES"))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}
