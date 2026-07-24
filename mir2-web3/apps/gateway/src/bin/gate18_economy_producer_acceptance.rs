use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use mir2_gateway::economy::{EconomyBalanceKey, PostgresEconomyStore};
use mir2_gateway::{
    GatewayConfig, PostgresEconomyAccountInventoryService, SharedAccountInventoryCommand,
    SharedAccountInventoryCommandEnvelope, SharedAccountInventoryExecutionContext,
    SharedAccountInventoryService, ZoneId,
};
use mir2_protocol::{ClientPacket, MirClass, MirGender, ServerPacket};
use mir2_simulation::{
    ActiveSessionIdentity, GroundDropLootSnapshot, GroundDropSnapshot, InProcessWorldRuntime,
    WorldCommand, WorldRuntime,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Gate18EconomyProducerEvidence {
    schema_version: u32,
    generated_at_ms: u64,
    run_id: String,
    zone_id: String,
    fencing_generation: u64,
    source_sequence: u64,
    active_gold_before: u32,
    active_gold_after: u32,
    standby_gold_before: u32,
    standby_gold_after: u32,
    ledger_gold_after: i64,
    assertions: BTreeMap<String, bool>,
    success: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = env::var("MIR2_ECONOMY_DATABASE_URL")
        .map_err(|_| "MIR2_ECONOMY_DATABASE_URL is required")?;
    let output =
        PathBuf::from(env::var("MIR2_GATE18_ECONOMY_OUT").unwrap_or_else(|_| {
            "docs/generated/regional/gate18-economy-producer.json".to_string()
        }));
    let generated_at_ms = now_ms();
    let run_id = format!("gate18-{generated_at_ms}");
    let account_id = format!("{run_id}-account");
    let character_name = format!("R{}", generated_at_ms % 1_000_000);
    let object_id = u32::try_from(generated_at_ms % 1_000_000_000).unwrap_or(1);
    let zone_id = ZoneId::new("regional:gate18");
    let fencing_generation = 18;
    let source_sequence = 700;

    let store = PostgresEconomyStore::new(database_url.clone());
    store.ensure_migrated()?;
    let active_service = PostgresEconomyAccountInventoryService::new(database_url.clone());
    active_service.ensure_migrated()?;
    let mut active_runtime = start_runtime(&account_id, &character_name)?;
    let identity = active_runtime
        .active_identity()
        .ok_or("active Gate 18 runtime has no identity")?;
    let command = gold_pickup(identity.clone(), object_id, 25);
    let active_context = SharedAccountInventoryExecutionContext {
        zone_id: zone_id.clone(),
        fencing_generation,
        source_sequence,
        created_at_ms: generated_at_ms,
        external_commit_authorized: true,
    };
    let active_gold_before = active_runtime.world_snapshot().gold;
    let first =
        active_service.commit_fenced(&mut active_runtime, Some(&active_context), command.clone());
    let active_gold_after = active_runtime.world_snapshot().gold;
    let retry =
        active_service.commit_fenced(&mut active_runtime, Some(&active_context), command.clone());
    let active_gold_after_retry = active_runtime.world_snapshot().gold;

    let balance = EconomyBalanceKey::gold(&account_id, identity.character_index);
    let ledger_gold_after_active = store.balance(&balance)?;

    // A different Zone Host owns a different producer instance. Verified
    // standby replay must rebuild its in-memory projection but must not write
    // the shared PostgreSQL ledger.
    let standby_service = PostgresEconomyAccountInventoryService::new(database_url);
    let mut standby_runtime = start_runtime(&account_id, &character_name)?;
    let standby_identity = standby_runtime
        .active_identity()
        .ok_or("standby Gate 18 runtime has no identity")?;
    if standby_identity.character_index != identity.character_index {
        return Err("active and standby character indexes differ".into());
    }
    let standby_command = gold_pickup(standby_identity, object_id, 25);
    let standby_context = SharedAccountInventoryExecutionContext {
        external_commit_authorized: false,
        ..active_context.clone()
    };
    let standby_gold_before = standby_runtime.world_snapshot().gold;
    let standby = standby_service.commit_fenced(
        &mut standby_runtime,
        Some(&standby_context),
        standby_command.clone(),
    );
    let standby_gold_after = standby_runtime.world_snapshot().gold;
    let ledger_gold_after_standby = store.balance(&balance)?;

    let mut unfenced_runtime = start_runtime(&format!("{account_id}-unfenced"), "Unfenced")?;
    let unfenced_identity = unfenced_runtime
        .active_identity()
        .ok_or("unfenced Gate 18 runtime has no identity")?;
    let unfenced = active_service.commit_fenced(
        &mut unfenced_runtime,
        None,
        gold_pickup(unfenced_identity, object_id.saturating_add(1), 25),
    );

    let assertions = BTreeMap::from([
        (
            "activeOwnerCommittedLedgerAndProjection".to_string(),
            first.committed
                && active_gold_after == active_gold_before.saturating_add(25)
                && ledger_gold_after_active == 25,
        ),
        (
            "activeRetryDidNotDoubleCredit".to_string(),
            retry.committed
                && retry.packets.is_empty()
                && active_gold_after_retry == active_gold_after
                && store.balance(&balance)? == 25,
        ),
        (
            "standbyReplayUpdatedProjectionOnly".to_string(),
            standby.committed
                && standby_gold_after == standby_gold_before.saturating_add(25)
                && ledger_gold_after_standby == ledger_gold_after_active,
        ),
        (
            "unfencedProducerRejected".to_string(),
            !unfenced.committed && unfenced.packets.is_empty(),
        ),
        (
            "ownerFenceAndSourceSequenceBound".to_string(),
            active_context.fencing_generation == fencing_generation
                && active_context.source_sequence == source_sequence
                && active_context.external_commit_authorized
                && !standby_context.external_commit_authorized,
        ),
    ]);
    let success = assertions.values().all(|value| *value);
    let evidence = Gate18EconomyProducerEvidence {
        schema_version: 1,
        generated_at_ms,
        run_id,
        zone_id: zone_id.as_str().to_string(),
        fencing_generation,
        source_sequence,
        active_gold_before,
        active_gold_after,
        standby_gold_before,
        standby_gold_after,
        ledger_gold_after: store.balance(&balance)?,
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
    println!("Wrote {}", output.display());
    if !success {
        std::process::exit(1);
    }
    Ok(())
}

fn start_runtime(account_id: &str, character_name: &str) -> Result<InProcessWorldRuntime, String> {
    let mut runtime = InProcessWorldRuntime::new(GatewayConfig::default());
    runtime.execute(WorldCommand::ClientPacket(ClientPacket::NewAccount {
        account_id: account_id.to_string(),
        password: account_id.to_string(),
        birth_date_binary: 0,
        user_name: String::new(),
        secret_question: String::new(),
        secret_answer: String::new(),
        email_address: String::new(),
    }))?;
    runtime.execute(WorldCommand::ClientPacket(ClientPacket::Login {
        account_id: account_id.to_string(),
        password: account_id.to_string(),
    }))?;
    let character_index = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::NewCharacter {
            name: character_name.to_string(),
            gender: MirGender::Male,
            class: MirClass::Warrior,
        }))?
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
            _ => None,
        })
        .ok_or_else(|| "Gate 18 character creation returned no index".to_string())?;
    runtime.execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
        character_index,
    }))?;
    Ok(runtime)
}

fn gold_pickup(
    identity: ActiveSessionIdentity,
    object_id: u32,
    amount: u32,
) -> SharedAccountInventoryCommandEnvelope {
    SharedAccountInventoryCommandEnvelope {
        identity,
        command: SharedAccountInventoryCommand::GroundDropPickup(GroundDropSnapshot {
            object_id,
            name: format!("{amount} Gold"),
            name_colour_argb: -1,
            icon: 0,
            x: 0,
            y: 0,
            quantity: amount,
            source_monster: "Gate18 Regional Producer".to_string(),
            owner_object_id: None,
            ownership_remaining_ticks: None,
            loot: GroundDropLootSnapshot::Gold { amount },
        }),
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}
