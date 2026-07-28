use std::env;
use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mir2_gateway::zone_lease::default_zone_owner_lease_authority_from_env;
use mir2_gateway::{GatewayConfig, GatewaySession, ZoneTopology};
use mir2_protocol::{ClientPacket, MirClass, MirGender, ServerPacket};
use mir2_simulation::WorldCommand;
use serde::Serialize;

const DEFAULT_OUTPUT: &str = "docs/generated/regional/gate19-zone-session.json";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Gate19ZoneSessionEvidence {
    schema_version: u32,
    generated_at_ms: u64,
    account_id: String,
    character_name: String,
    character_index: i32,
    session_id: String,
    zone_id: String,
    initial_owner: String,
    initial_generation: u64,
    promoted_owner: String,
    promoted_generation: u64,
    initial_tick: u64,
    promoted_tick: u64,
    initial_map: Option<String>,
    promoted_map: Option<String>,
    identity_preserved: bool,
    observed_failures: u64,
    waiting_for_promotion_ms: f64,
    resume_command_ms: f64,
    success: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = PathBuf::from(
        env::var("MIR2_GATE19_ZONE_SESSION_OUT").unwrap_or_else(|_| DEFAULT_OUTPUT.to_string()),
    );
    let promoted_owner =
        env::var("MIR2_ZONE_STANDBY_OWNER_ID").unwrap_or_else(|_| "gate19-standby".to_string());
    let maximum_wait = Duration::from_millis(env_u64(
        "MIR2_GATE19_ZONE_SESSION_MAX_WAIT_MS",
        30_000,
        1_000,
        300_000,
    ));
    let generated_at_ms = now_ms();
    let account_id = format!("g19s{:012}", generated_at_ms % 1_000_000_000_000);
    let character_name = format!("S{:011}", generated_at_ms % 100_000_000_000);
    let topology = ZoneTopology::from_env()?;
    let registry = topology.zone_registry(default_zone_owner_lease_authority_from_env());
    let mut session = GatewaySession::new_with_zone_registry(
        GatewayConfig::default().with_crystal_world_runtime(),
        &registry,
    );
    if session.on_connect().is_empty() {
        return Err("Zone Host returned no connect packets".into());
    }
    session.handle_packet(ClientPacket::NewAccount {
        account_id: account_id.clone(),
        password: account_id.clone(),
        birth_date_binary: 0,
        user_name: String::new(),
        secret_question: String::new(),
        secret_answer: String::new(),
        email_address: String::new(),
    });
    session.handle_packet(ClientPacket::Login {
        account_id: account_id.clone(),
        password: account_id.clone(),
    });
    let character_index = session
        .handle_packet(ClientPacket::NewCharacter {
            name: character_name.clone(),
            gender: MirGender::Male,
            class: MirClass::Warrior,
        })
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
            _ => None,
        })
        .ok_or("Zone Host returned no character creation index")?;
    let packets = session.handle_packet(ClientPacket::StartGame { character_index });
    if !packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. }))
    {
        return Err("character did not enter the world".into());
    }
    let initial_owner = session.zone_owner_lease().owner_id().to_string();
    let initial_generation = session.zone_owner_lease().fencing_token();
    let initial_snapshot = session.world_snapshot();
    let initial_tick = initial_snapshot.tick;
    let initial_map = initial_snapshot.map_file_name;
    let initial_identity = session.active_identity();
    println!(
        "GATE19_ZONE_SEED_READY session={} zone={} owner={} generation={} tick={}",
        session.session_id(),
        session.zone_id(),
        initial_owner,
        initial_generation,
        initial_tick
    );

    let started = Instant::now();
    let mut observed_failures = 0_u64;
    let (
        final_owner,
        final_generation,
        promoted_tick,
        promoted_map,
        final_identity,
        resume_command_ms,
    ) = loop {
        if started.elapsed() > maximum_wait {
            return Err(format!(
                "player did not resume on {promoted_owner} within {} ms",
                maximum_wait.as_millis()
            )
            .into());
        }
        match session.refresh_zone_owner_lease() {
            Ok(lease)
                if lease.owner_id() == promoted_owner
                    && lease.fencing_token() > initial_generation =>
            {
                let final_owner = lease.owner_id().to_string();
                let final_generation = lease.fencing_token();
                let resume_started = Instant::now();
                match session.execute_production_player_command(
                    true,
                    WorldCommand::ClientPacket(ClientPacket::KeepAlive {
                        time: now_ms().try_into().unwrap_or(i64::MAX),
                    }),
                ) {
                    Ok(_) => {
                        let promoted_snapshot = session.world_snapshot();
                        break (
                            final_owner,
                            final_generation,
                            promoted_snapshot.tick,
                            promoted_snapshot.map_file_name,
                            session.active_identity(),
                            resume_started.elapsed().as_secs_f64() * 1_000.0,
                        );
                    }
                    Err(error) => {
                        observed_failures = observed_failures.saturating_add(1);
                        eprintln!("Gate 19 player waiting for promoted Zone: {error}");
                    }
                }
            }
            Ok(_) => {}
            Err(error) => {
                observed_failures = observed_failures.saturating_add(1);
                eprintln!("Gate 19 player waiting for promoted lease: {error}");
            }
        }
        thread::sleep(Duration::from_millis(20));
    };
    let identity_preserved = final_identity == initial_identity;
    let state_preserved = promoted_map == initial_map && identity_preserved;
    let evidence = Gate19ZoneSessionEvidence {
        schema_version: 1,
        generated_at_ms: now_ms(),
        account_id,
        character_name,
        character_index,
        session_id: session.session_id().to_string(),
        zone_id: session.zone_id().as_str().to_string(),
        initial_owner,
        initial_generation,
        promoted_owner: final_owner,
        promoted_generation: final_generation,
        initial_tick,
        promoted_tick,
        initial_map,
        promoted_map,
        identity_preserved,
        observed_failures,
        waiting_for_promotion_ms: started.elapsed().as_secs_f64() * 1_000.0,
        resume_command_ms,
        success: final_generation > initial_generation && state_preserved,
    };
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, serde_json::to_vec_pretty(&evidence)?)?;
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    if evidence.success {
        Ok(())
    } else {
        Err("Gate 19 real-session continuity assertions failed".into())
    }
}

fn env_u64(name: &str, default: u64, minimum: u64, maximum: u64) -> u64 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
        .clamp(minimum, maximum)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}
