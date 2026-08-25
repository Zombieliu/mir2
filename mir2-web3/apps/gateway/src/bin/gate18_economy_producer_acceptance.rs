use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use mir2_gateway::economy::{EconomyBalanceKey, PostgresEconomyStore};
use mir2_gateway::routing::SharedAccountInventoryCommitOutcome;
use mir2_gateway::{
    GatewayConfig, PostgresEconomyAccountInventoryService, SharedAccountInventoryCommand,
    SharedAccountInventoryCommandEnvelope, SharedAccountInventoryExecutionContext,
    SharedAccountInventoryService, SharedTradeSettlementOutcome, ZoneId,
};
use mir2_protocol::{ClientPacket, MirClass, MirGender, ServerPacket, Spell};
use mir2_simulation::{
    ActiveSessionIdentity, GroundDropLootSnapshot, GroundDropSnapshot, InProcessWorldRuntime,
    SharedTradeOffer, SharedTradeOfferItem, WorldCommand, WorldRuntime,
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
    recovered_gold_before: u32,
    recovered_gold_after: u32,
    materialized_gold_before: u32,
    materialized_gold_after: u32,
    ledger_gold_after: i64,
    bootstrap_opening_gold: i64,
    trade_item_key: String,
    trade_alice_gold_after: i64,
    trade_bob_gold_after: i64,
    trade_alice_item_after: i64,
    trade_bob_item_after: i64,
    skill_amulet_key: String,
    skill_poison_key: String,
    skill_amulet_ledger_after: i64,
    skill_poison_ledger_after: i64,
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
    let standby_service = PostgresEconomyAccountInventoryService::new(database_url.clone());
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

    // Simulate the crash window after PostgreSQL commit but before the active
    // Host could checkpoint its runtime projection. A fresh producer must
    // replay the projection from the duplicate receipt without crediting the
    // ledger a second time.
    let recovery_service = PostgresEconomyAccountInventoryService::new(database_url.clone());
    let mut recovery_runtime = start_runtime(&account_id, &character_name)?;
    let recovery_identity = recovery_runtime
        .active_identity()
        .ok_or("recovery Gate 18 runtime has no identity")?;
    let recovery_command = gold_pickup(recovery_identity, object_id, 25);
    let recovered_gold_before = recovery_runtime.world_snapshot().gold;
    let recovery_context = SharedAccountInventoryExecutionContext {
        fencing_generation: active_context.fencing_generation.saturating_add(100),
        source_sequence: active_context.source_sequence.saturating_add(10_000),
        created_at_ms: active_context.created_at_ms.saturating_add(60_000),
        ..active_context.clone()
    };
    let recovered = recovery_service.commit_fenced(
        &mut recovery_runtime,
        Some(&recovery_context),
        recovery_command,
    );
    let recovered_gold_after = recovery_runtime.world_snapshot().gold;
    let ledger_gold_after_recovery = store.balance(&balance)?;

    // The opposite restart window restores a checkpoint that already includes
    // the projection. A fresh producer must recognize parity with PostgreSQL
    // and must not apply the duplicate credit again.
    let materialized_service = PostgresEconomyAccountInventoryService::new(database_url.clone());
    let materialized_gold_before = active_runtime.world_snapshot().gold;
    let materialized = materialized_service.commit_fenced(
        &mut active_runtime,
        Some(&active_context),
        command.clone(),
    );
    let materialized_gold_after = active_runtime.world_snapshot().gold;
    let ledger_gold_after_materialized = store.balance(&balance)?;

    let active_bootstrap =
        store.bootstrap_character(&identity, &active_runtime.world_snapshot(), generated_at_ms)?;
    let bob_account_id = format!("{run_id}-bob-account");
    let mut bob_runtime = start_runtime(&bob_account_id, "RegionalBob")?;
    let bob_identity = bob_runtime
        .active_identity()
        .ok_or("Bob Gate 18 runtime has no identity")?;
    let bob_context = SharedAccountInventoryExecutionContext {
        source_sequence: source_sequence.saturating_add(1),
        ..active_context.clone()
    };
    let bob_bootstrapped = active_service.bootstrap_fenced(&bob_runtime, Some(&bob_context));
    let alice_item = active_runtime
        .world_snapshot()
        .inventory_items
        .first()
        .cloned()
        .ok_or("Gate 18 trade requires a starter inventory item")?;
    let trade_item_key = alice_item.key.clone();
    let alice_item_balance =
        EconomyBalanceKey::item_quantity(&account_id, identity.character_index, &trade_item_key);
    let bob_item_balance = EconomyBalanceKey::item_quantity(
        &bob_account_id,
        bob_identity.character_index,
        &trade_item_key,
    );
    let bob_gold_balance = EconomyBalanceKey::gold(&bob_account_id, bob_identity.character_index);
    let alice_gold_before_trade = store.balance(&balance)?;
    let bob_gold_before_trade = store.balance(&bob_gold_balance)?;
    let alice_item_before_trade = store.balance(&alice_item_balance)?;
    let bob_item_before_trade = store.balance(&bob_item_balance)?;
    let alice_offer = SharedTradeOffer {
        settlement_nonce: "00000000000000000000000000000001".to_string(),
        account_id: account_id.clone(),
        character_index: identity.character_index,
        character_name: character_name.clone(),
        partner_name: bob_identity.character_name.clone(),
        gold: 10,
        items: vec![SharedTradeOfferItem {
            item_state_json: serde_json::to_string(&alice_item)?,
            key: trade_item_key.clone(),
            unique_id: alice_item.unique_id,
        }],
    };
    let bob_offer = SharedTradeOffer {
        settlement_nonce: "00000000000000000000000000000002".to_string(),
        account_id: bob_account_id.clone(),
        character_index: bob_identity.character_index,
        character_name: bob_identity.character_name.clone(),
        partner_name: character_name.clone(),
        gold: 0,
        items: Vec::new(),
    };
    let trade = active_service.settle_trade_fenced(Some(&bob_context), &alice_offer, &bob_offer);
    let trade_alice_gold_after = store.balance(&balance)?;
    let trade_bob_gold_after = store.balance(&bob_gold_balance)?;
    let trade_alice_item_after = store.balance(&alice_item_balance)?;
    let trade_bob_item_after = store.balance(&bob_item_balance)?;
    // A retry from a successor producer has a new attempt fence/sequence/time
    // but must recover the same trade settlement nonce pair and event id.
    let trade_retry_context = SharedAccountInventoryExecutionContext {
        fencing_generation: bob_context.fencing_generation.saturating_add(100),
        source_sequence: bob_context.source_sequence.saturating_add(10_000),
        created_at_ms: bob_context.created_at_ms.saturating_add(60_000),
        ..bob_context.clone()
    };
    let trade_retry =
        active_service.settle_trade_fenced(Some(&trade_retry_context), &alice_offer, &bob_offer);
    let standby_trade = active_service.settle_trade_fenced(
        Some(&SharedAccountInventoryExecutionContext {
            external_commit_authorized: false,
            ..bob_context.clone()
        }),
        &alice_offer,
        &bob_offer,
    );
    let unfenced_trade = active_service.settle_trade_fenced(None, &alice_offer, &bob_offer);

    let alice_private_gold_before_trade = active_runtime.world_snapshot().gold;
    let bob_private_gold_before_trade = bob_runtime.world_snapshot().gold;
    let expected_trade_item_count = u16::try_from(alice_item.quantity)?;
    let alice_trade_packets =
        active_service.reconcile_trade_projections_fenced(&mut active_runtime, Some(&bob_context));
    let bob_trade_packets =
        active_service.reconcile_trade_projections_fenced(&mut bob_runtime, Some(&bob_context));
    let alice_trade_snapshot = active_runtime.world_snapshot();
    let bob_trade_snapshot = bob_runtime.world_snapshot();
    let alice_trade_replay =
        active_service.reconcile_trade_projections_fenced(&mut active_runtime, Some(&bob_context));
    let bob_trade_replay =
        active_service.reconcile_trade_projections_fenced(&mut bob_runtime, Some(&bob_context));
    let alice_trade_pending =
        active_service.has_pending_trade_projection_fenced(&active_runtime, Some(&bob_context));
    let bob_trade_pending =
        active_service.has_pending_trade_projection_fenced(&bob_runtime, Some(&bob_context));

    let skill_account_id = format!("{run_id}-skill-account");
    let mut skill_runtime = start_runtime(&skill_account_id, "RegionalTaoist")?;
    configure_poison_cloud_fixture(&mut skill_runtime)?;
    let skill_identity = skill_runtime
        .active_identity()
        .ok_or("skill Gate 18 runtime has no identity")?;
    let skill_components = skill_runtime
        .shared_skill_item_consumption_components(Spell::PoisonCloud)
        .ok_or("skill Gate 18 runtime cannot resolve exact consumption components")?;
    if skill_components.len() != 2 {
        return Err("PoisonCloud must resolve two economy components".into());
    }
    let skill_amulet_key = skill_components[0].item_key.clone();
    let skill_poison_key = skill_components[1].item_key.clone();
    let skill_context = SharedAccountInventoryExecutionContext {
        source_sequence: source_sequence.saturating_add(2),
        ..active_context.clone()
    };
    let skill_command = SharedAccountInventoryCommandEnvelope {
        identity: skill_identity.clone(),
        command: SharedAccountInventoryCommand::SkillItemConsume {
            spell: Spell::PoisonCloud,
            request_id: 1,
            components: skill_components,
        },
    };
    let skill_amulet_balance = EconomyBalanceKey::item_quantity(
        &skill_account_id,
        skill_identity.character_index,
        &skill_amulet_key,
    );
    let skill_poison_balance = EconomyBalanceKey::item_quantity(
        &skill_account_id,
        skill_identity.character_index,
        &skill_poison_key,
    );
    let skill = active_service.commit_fenced(
        &mut skill_runtime,
        Some(&skill_context),
        skill_command.clone(),
    );
    let skill_amulet_ledger_after = store.balance(&skill_amulet_balance)?;
    let skill_poison_ledger_after = store.balance(&skill_poison_balance)?;
    let skill_retry =
        active_service.commit_fenced(&mut skill_runtime, Some(&skill_context), skill_command);
    let skill_snapshot_after = skill_runtime.world_snapshot();

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
                && ledger_gold_after_active == 25,
        ),
        (
            "standbyReplayUpdatedProjectionOnly".to_string(),
            standby.committed
                && standby_gold_after == standby_gold_before.saturating_add(25)
                && ledger_gold_after_standby == ledger_gold_after_active,
        ),
        (
            "crashRecoveryReplayedProjectionWithoutLedgerDuplication".to_string(),
            recovered.committed
                && recovered_gold_after == recovered_gold_before.saturating_add(25)
                && ledger_gold_after_recovery == ledger_gold_after_active,
        ),
        (
            "restoredCheckpointDidNotReplayMaterializedProjection".to_string(),
            materialized.committed
                && materialized_gold_after == materialized_gold_before
                && ledger_gold_after_materialized == ledger_gold_after_active,
        ),
        (
            "unfencedProducerDeferred".to_string(),
            matches!(
                unfenced,
                SharedAccountInventoryCommitOutcome::Deferred { ref receipt }
                    if !receipt.committed && receipt.packets.is_empty()
            ),
        ),
        (
            "ownerFenceAndSourceSequenceBound".to_string(),
            active_context.fencing_generation == fencing_generation
                && active_context.source_sequence == source_sequence
                && active_context.external_commit_authorized
                && !standby_context.external_commit_authorized,
        ),
        (
            "legacyOpeningBalancesBootstrappedOnce".to_string(),
            active_bootstrap.duplicate
                && active_bootstrap.gold == i64::from(active_gold_before)
                && bob_bootstrapped,
        ),
        (
            "twoSidedTradeConservedGoldAndItems".to_string(),
            matches!(trade, SharedTradeSettlementOutcome::DurableCommitted { .. })
                && trade_alice_gold_after == alice_gold_before_trade - 10
                && trade_bob_gold_after == bob_gold_before_trade + 10
                && trade_alice_item_after == alice_item_before_trade - 1
                && trade_bob_item_after == bob_item_before_trade + 1,
        ),
        (
            "tradeRetryAndStandbyDidNotDoubleSettle".to_string(),
            matches!(
                trade_retry,
                SharedTradeSettlementOutcome::DurableDuplicate { .. }
            ) && standby_trade == SharedTradeSettlementOutcome::Committed
                && store.balance(&balance)? == trade_alice_gold_after
                && store.balance(&bob_gold_balance)? == trade_bob_gold_after
                && store.balance(&alice_item_balance)? == trade_alice_item_after
                && store.balance(&bob_item_balance)? == trade_bob_item_after,
        ),
        (
            "unfencedTradeDeferred".to_string(),
            unfenced_trade == SharedTradeSettlementOutcome::Deferred,
        ),
        (
            "tradePrivateProjectionsMaterializedExactlyOnce".to_string(),
            alice_trade_packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::LoseGold { gold: 10 }
            )) && alice_trade_packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::DeleteItem { unique_id, count }
                    if *unique_id == alice_item.unique_id && *count == expected_trade_item_count
            )) && bob_trade_packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::GainedGold { gold: 10 }
            )) && bob_trade_packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::GainedItem { item } if item.unique_id == alice_item.unique_id
            )) && alice_trade_snapshot.gold == alice_private_gold_before_trade - 10
                && bob_trade_snapshot.gold == bob_private_gold_before_trade + 10
                && !alice_trade_snapshot
                    .inventory_items
                    .iter()
                    .any(|item| item.unique_id == alice_item.unique_id)
                && bob_trade_snapshot
                    .inventory_items
                    .iter()
                    .any(|item| item.unique_id == alice_item.unique_id)
                && alice_trade_replay.is_empty()
                && bob_trade_replay.is_empty()
                && !alice_trade_pending
                && !bob_trade_pending,
        ),
        (
            "exactSkillComponentsDebitedOnce".to_string(),
            skill.committed
                && skill
                    .packets
                    .iter()
                    .filter(|packet| matches!(packet, ServerPacket::DeleteItem { count: 5, .. }))
                    .count()
                    == 2
                && skill_amulet_ledger_after == 0
                && skill_poison_ledger_after == 0
                && skill_snapshot_after
                    .equipment_items
                    .iter()
                    .all(|item| item.key != skill_amulet_key && item.key != skill_poison_key)
                && skill_retry.committed
                && skill_retry.packets.is_empty()
                && store.balance(&skill_amulet_balance)? == 0
                && store.balance(&skill_poison_balance)? == 0,
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
        recovered_gold_before,
        recovered_gold_after,
        materialized_gold_before,
        materialized_gold_after,
        ledger_gold_after: ledger_gold_after_standby,
        bootstrap_opening_gold: active_bootstrap.gold,
        trade_item_key,
        trade_alice_gold_after,
        trade_bob_gold_after,
        trade_alice_item_after,
        trade_bob_item_after,
        skill_amulet_key,
        skill_poison_key,
        skill_amulet_ledger_after,
        skill_poison_ledger_after,
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

fn configure_poison_cloud_fixture(runtime: &mut InProcessWorldRuntime) -> Result<(), String> {
    let snapshot = runtime.world_snapshot();
    let identity = runtime
        .active_identity()
        .ok_or_else(|| "skill fixture requires an active identity".to_string())?;
    let player = snapshot
        .entities
        .iter()
        .find(|entity| entity.kind == mir2_simulation::WorldEntityKind::SelfPlayer)
        .ok_or_else(|| "skill fixture requires an in-world player".to_string())?;
    let equipment_items_json = [
        ("Amulet", mir2_simulation::EquipmentSlot::Amulet),
        ("GreenPoison", mir2_simulation::EquipmentSlot::BraceletRight),
    ]
    .into_iter()
    .map(|(name, slot)| {
        let template = mir2_game_data::crystal_item_by_name(name)
            .ok_or_else(|| format!("missing Crystal skill fixture item {name}"))?;
        Ok(serde_json::json!({
            "key": format!("crystal-item-{}", template.item_index),
            "slot": slot,
            "quantity": 5,
            "name": template.name,
            "icon": template.image,
            "shape": u16::try_from(template.shape).ok(),
            "description": template.tooltip.unwrap_or_default(),
            "durability_current": template.durability.max(1),
            "durability_max": template.durability.max(1),
            "attack": 0,
            "defence": 0
        })
        .to_string())
    })
    .collect::<Result<Vec<_>, String>>()?;
    let state = serde_json::json!({
        "character": {
            "name": identity.character_name,
            "level": 48,
            "class": "Taoist",
            "gender": "Male"
        },
        "mapFileName": snapshot.map_file_name.unwrap_or_else(|| "0".to_string()),
        "mapTitle": snapshot.map_title.unwrap_or_else(|| "BichonProvince".to_string()),
        "position": { "x": player.x, "y": player.y },
        "direction": player.direction,
        "hp": snapshot.player_hp.unwrap_or(100),
        "maxHp": snapshot.player_max_hp.unwrap_or(100),
        "mp": snapshot.player_mp.unwrap_or(100),
        "maxMp": snapshot.player_max_mp.unwrap_or(100),
        "experience": snapshot.player_experience,
        "maxExperience": snapshot.player_max_experience,
        "gold": snapshot.gold,
        "credit": snapshot.credit,
        "inventoryItemsJson": [],
        "beltItemsJson": [],
        "storageItemsJson": [],
        "equipmentItemsJson": equipment_items_json
    });
    runtime.execute(WorldCommand::Stage5Command {
        action: "qa.applyNativeState".to_string(),
        args: vec![state.to_string()],
    })?;
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
    runtime.execute(WorldCommand::Stage5Command {
        action: "qa.giveItem".to_string(),
        args: vec!["red-potion".to_string(), "2".to_string()],
    })?;
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
