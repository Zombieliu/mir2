use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use mir2_gateway::economy::{EconomyBalanceKey, PostgresEconomyStore};
use mir2_gateway::zone_lease::default_zone_owner_lease_authority_from_env;
use mir2_gateway::{GatewayConfig, GatewaySession, TcpZoneOwnerRpcTransport, ZoneId, ZoneTopology};
use mir2_protocol::{ClientPacket, MirClass, MirGender, ServerPacket};
use serde::Serialize;

const PICKUP_GOLD: u32 = 25;
const FIXTURE_GOLD: u32 = 100;
const FIXTURE_ITEM_KEY: &str = "red-potion";
const FIXTURE_ITEM_QUANTITY: u32 = 3;
const DROP_ITEM_QUANTITY: u16 = 2;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Gate18RemoteEconomyEvidence {
    schema_version: u32,
    generated_at_ms: u64,
    run_id: String,
    gateway_process_id: u32,
    zone_host_endpoint: String,
    zone_host_id: String,
    zone_host_process_id: u32,
    zone_id: String,
    account_id: String,
    character_index: i32,
    fixture_gold: u32,
    fixture_item_key: String,
    fixture_item_quantity: u32,
    initial_gold: u32,
    initial_item_quantity: u32,
    drop_packet_observed: bool,
    item_drop_packet_observed: bool,
    gold_after_drop: u32,
    gold_after_pickup: u32,
    gold_after_retry: u32,
    item_quantity_after_drop: u32,
    item_quantity_after_pickup: u32,
    item_quantity_after_retry: u32,
    bootstrap_opening_gold: i64,
    bootstrap_opening_item_quantity: i64,
    ledger_gold_after_drop: i64,
    ledger_gold_after_pickup: i64,
    ledger_gold_after_retry: i64,
    ledger_item_quantity_after_drop: i64,
    ledger_item_quantity_after_pickup: i64,
    ledger_item_quantity_after_retry: i64,
    assertions: BTreeMap<String, bool>,
    success: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = env::var("MIR2_ECONOMY_DATABASE_URL")
        .map_err(|_| "MIR2_ECONOMY_DATABASE_URL is required")?;
    let output = PathBuf::from(
        env::var("MIR2_GATE18_REMOTE_ECONOMY_OUT")
            .unwrap_or_else(|_| "docs/generated/regional/gate18-remote-economy.json".to_string()),
    );
    let generated_at_ms = now_ms();
    let zone_host_endpoint = env::var("MIR2_ZONE_HOST_ADDR")
        .or_else(|_| env::var("MIR2_ZONE_HOST_ADDRS"))
        .map_err(|_| "MIR2_ZONE_HOST_ADDR or MIR2_ZONE_HOST_ADDRS is required")?;
    let run_id = format!("gate18-remote-{generated_at_ms}");
    let account_id = format!("g18r{}", generated_at_ms % 1_000_000_000);
    let character_name = format!("R{}", generated_at_ms % 1_000_000);

    let store = PostgresEconomyStore::new(database_url);
    store.ensure_migrated()?;

    let health_probe = TcpZoneOwnerRpcTransport::from_env(ZoneId::primary())
        .ok_or("MIR2_ZONE_HOST_ADDR or MIR2_ZONE_HOST_ADDRS is required")?;
    let host_before = health_probe.health()?;

    let topology = ZoneTopology::from_env()?;
    let registry = topology.zone_registry(default_zone_owner_lease_authority_from_env());
    let mut session = GatewaySession::new_with_zone_registry(
        GatewayConfig::default().with_crystal_world_runtime(),
        &registry,
    );
    if session.on_connect().is_empty() {
        return Err("remote Zone Host returned no connect packets".into());
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
            name: character_name,
            gender: MirGender::Male,
            class: MirClass::Warrior,
        })
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
            _ => None,
        })
        .ok_or("remote character creation returned no index")?;
    let start_packets = session.handle_packet(ClientPacket::StartGame { character_index });
    if !start_packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. }))
    {
        return Err("remote character did not enter the world".into());
    }
    let identity = session
        .active_identity()
        .ok_or("remote session has no active identity")?;
    let fixture_map = session
        .world_snapshot()
        .map_file_name
        .unwrap_or_else(|| "0".to_string());
    let fixture_position = if fixture_map == "0" {
        (330, 270)
    } else {
        (100, 100)
    };
    session.transfer_map(&format!(
        "crystal:{fixture_map}:{}:{}",
        fixture_position.0, fixture_position.1
    ));
    apply_gold_fixture(&mut session, &identity.character_name)?;
    session.stage5_command(
        "qa.giveItem",
        vec![
            FIXTURE_ITEM_KEY.to_string(),
            FIXTURE_ITEM_QUANTITY.to_string(),
        ],
    );
    let initial_gold = session.world_snapshot().gold;
    let initial_item = session
        .world_snapshot()
        .inventory_items
        .into_iter()
        .find(|item| item.key == FIXTURE_ITEM_KEY)
        .ok_or("remote QA item fixture was not applied")?;
    let initial_item_quantity = item_quantity(&session, FIXTURE_ITEM_KEY);

    let drop_packets = session.handle_packet(ClientPacket::DropGold {
        amount: PICKUP_GOLD,
    });
    let drop_packet_observed = drop_packets.iter().any(|packet| {
        matches!(
            packet,
            ServerPacket::ObjectGold { info } if info.gold == PICKUP_GOLD
        )
    });
    let gold_after_drop = session.world_snapshot().gold;
    let balance_key = EconomyBalanceKey::gold(&account_id, character_index);
    let ledger_gold_after_drop = store.balance(&balance_key)?;
    let dropped = session
        .world_snapshot()
        .ground_drops
        .into_iter()
        .find(|drop| {
            matches!(
                drop.loot,
                mir2_simulation::GroundDropLootSnapshot::Gold { amount }
                    if amount == PICKUP_GOLD
            )
        })
        .ok_or("remote gold drop is missing from the Zone snapshot")?;
    let current_map = session
        .world_snapshot()
        .map_file_name
        .unwrap_or_else(|| "0".to_string());
    session.transfer_map(&format!(
        "crystal:{current_map}:{}:{}",
        dropped.x, dropped.y
    ));
    let pickup_packets = session.handle_packet(ClientPacket::PickUp);
    if !pickup_packets.iter().any(|packet| {
        matches!(
            packet,
            ServerPacket::GainedGold { gold } if *gold == PICKUP_GOLD
        )
    }) {
        return Err("remote PickUp did not return GainedGold".into());
    }
    let gold_after_pickup = session.world_snapshot().gold;
    let ledger_gold_after_pickup = store.balance(&balance_key)?;

    // A real client may repeat PickUp after losing the response. The Zone no
    // longer contains the claimed object, so the runtime and ledger must both
    // remain unchanged.
    session.handle_packet(ClientPacket::PickUp);
    let gold_after_retry = session.world_snapshot().gold;
    let ledger_gold_after_retry = store.balance(&balance_key)?;

    let item_balance_key =
        EconomyBalanceKey::item_quantity(&account_id, character_index, FIXTURE_ITEM_KEY);
    let item_drop_packets = session.handle_packet(ClientPacket::DropItem {
        unique_id: initial_item.unique_id,
        count: DROP_ITEM_QUANTITY,
        hero_inventory: false,
    });
    let item_drop_packet_observed = item_drop_packets.iter().any(|packet| {
        matches!(
            packet,
            ServerPacket::DropItem {
                unique_id,
                count: DROP_ITEM_QUANTITY,
                hero_inventory: false,
                success: true,
            } if *unique_id == initial_item.unique_id
        )
    });
    let item_quantity_after_drop = item_quantity(&session, FIXTURE_ITEM_KEY);
    let ledger_item_quantity_after_drop = store.balance(&item_balance_key)?;
    let dropped_item = session
        .world_snapshot()
        .ground_drops
        .into_iter()
        .find(|drop| {
            drop.quantity == u32::from(DROP_ITEM_QUANTITY)
                && matches!(
                    &drop.loot,
                    mir2_simulation::GroundDropLootSnapshot::InventoryItem { key, .. }
                        if key == FIXTURE_ITEM_KEY
                )
        })
        .ok_or("remote item drop is missing from the Zone snapshot")?;
    session.transfer_map(&format!(
        "crystal:{current_map}:{}:{}",
        dropped_item.x, dropped_item.y
    ));
    let item_pickup_packets = session.handle_packet(ClientPacket::PickUp);
    if !item_pickup_packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::GainedItem { .. }))
    {
        return Err("remote item PickUp did not return GainedItem".into());
    }
    let item_quantity_after_pickup = item_quantity(&session, FIXTURE_ITEM_KEY);
    let ledger_item_quantity_after_pickup = store.balance(&item_balance_key)?;
    session.handle_packet(ClientPacket::PickUp);
    let item_quantity_after_retry = item_quantity(&session, FIXTURE_ITEM_KEY);
    let ledger_item_quantity_after_retry = store.balance(&item_balance_key)?;
    let bootstrap =
        store.bootstrap_character(&identity, &session.world_snapshot(), generated_at_ms)?;
    let host_after = health_probe.health()?;

    let gateway_process_id = std::process::id();
    let assertions = BTreeMap::from([
        (
            "gatewayUsesSeparateZoneHostContainer".to_string(),
            !zone_host_endpoint.starts_with("127.0.0.1:")
                && !zone_host_endpoint.starts_with("localhost:")
                && host_after.host_id == "gate18-regional-owner",
        ),
        (
            "authenticatedRemoteZoneHostHandledSession".to_string(),
            host_after.host_id == host_before.host_id
                && host_after.session_count >= 1
                && host_after.zone_count >= 1,
        ),
        (
            "realClientDropAndPickupRoundTripped".to_string(),
            gold_after_drop == initial_gold.saturating_sub(PICKUP_GOLD)
                && gold_after_pickup == initial_gold,
        ),
        (
            "postgresOpeningBalanceCapturedBeforeDrop".to_string(),
            bootstrap.gold == i64::from(initial_gold),
        ),
        (
            "postgresLedgerDebitedPlayerGoldDrop".to_string(),
            ledger_gold_after_drop == i64::from(gold_after_drop),
        ),
        (
            "postgresLedgerMatchesFinalRuntimeGold".to_string(),
            ledger_gold_after_pickup == i64::from(gold_after_pickup),
        ),
        (
            "realClientItemDropAndPickupRoundTripped".to_string(),
            item_drop_packet_observed
                && item_quantity_after_drop
                    == initial_item_quantity.saturating_sub(u32::from(DROP_ITEM_QUANTITY))
                && item_quantity_after_pickup == initial_item_quantity,
        ),
        (
            "postgresOpeningItemBalanceCapturedBeforeDrop".to_string(),
            bootstrap.item_quantity == i64::from(initial_item_quantity),
        ),
        (
            "postgresLedgerDebitedPlayerItemDrop".to_string(),
            ledger_item_quantity_after_drop == i64::from(item_quantity_after_drop),
        ),
        (
            "postgresLedgerMatchesFinalRuntimeItemQuantity".to_string(),
            ledger_item_quantity_after_pickup == i64::from(item_quantity_after_pickup),
        ),
        (
            "clientRetryDidNotDuplicateGold".to_string(),
            gold_after_retry == gold_after_pickup
                && ledger_gold_after_retry == ledger_gold_after_pickup,
        ),
        (
            "clientRetryDidNotDuplicateItem".to_string(),
            item_quantity_after_retry == item_quantity_after_pickup
                && ledger_item_quantity_after_retry == ledger_item_quantity_after_pickup,
        ),
        (
            "remoteGroundObjectWasConsumed".to_string(),
            !session.world_snapshot().ground_drops.iter().any(|drop| {
                drop.object_id == dropped.object_id || drop.object_id == dropped_item.object_id
            }),
        ),
    ]);
    let success = assertions.values().all(|passed| *passed);
    let evidence = Gate18RemoteEconomyEvidence {
        schema_version: 1,
        generated_at_ms,
        run_id,
        gateway_process_id,
        zone_host_endpoint,
        zone_host_id: host_after.host_id,
        zone_host_process_id: host_after.process_id,
        zone_id: session.zone_id().as_str().to_string(),
        account_id,
        character_index,
        fixture_gold: FIXTURE_GOLD,
        fixture_item_key: FIXTURE_ITEM_KEY.to_string(),
        fixture_item_quantity: FIXTURE_ITEM_QUANTITY,
        initial_gold,
        initial_item_quantity,
        drop_packet_observed,
        item_drop_packet_observed,
        gold_after_drop,
        gold_after_pickup,
        gold_after_retry,
        item_quantity_after_drop,
        item_quantity_after_pickup,
        item_quantity_after_retry,
        bootstrap_opening_gold: bootstrap.gold,
        bootstrap_opening_item_quantity: bootstrap.item_quantity,
        ledger_gold_after_drop,
        ledger_gold_after_pickup,
        ledger_gold_after_retry,
        ledger_item_quantity_after_drop,
        ledger_item_quantity_after_pickup,
        ledger_item_quantity_after_retry,
        assertions,
        success,
    };
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, serde_json::to_vec_pretty(&evidence)?)?;
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    if !success {
        return Err("Gate 18 remote economy acceptance failed".into());
    }
    Ok(())
}

fn item_quantity(session: &GatewaySession, item_key: &str) -> u32 {
    session
        .world_snapshot()
        .inventory_items
        .iter()
        .filter(|item| item.key == item_key)
        .map(|item| item.quantity)
        .sum()
}

fn apply_gold_fixture(
    session: &mut GatewaySession,
    character_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let snapshot = session.world_snapshot();
    let player = snapshot
        .entities
        .iter()
        .find(|entity| entity.kind == mir2_simulation::WorldEntityKind::SelfPlayer)
        .ok_or("remote gold fixture requires an in-world player")?;
    let state = serde_json::json!({
        "character": {
            "name": character_name,
            "level": 1,
            "class": "Warrior",
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
        "gold": FIXTURE_GOLD,
        "credit": snapshot.credit,
        "inventoryItemsJson": [],
        "beltItemsJson": [],
        "storageItemsJson": [],
        "equipmentItemsJson": []
    });
    session.stage5_command("qa.applyNativeState", vec![state.to_string()]);
    if session.world_snapshot().gold != FIXTURE_GOLD {
        return Err("remote QA gold fixture was not applied".into());
    }
    Ok(())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}
