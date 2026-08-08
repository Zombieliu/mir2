use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mir2_gateway::economy::{EconomyBalanceKey, PostgresEconomyStore};
use mir2_gateway::zone_lease::default_zone_owner_lease_authority_from_env;
use mir2_gateway::{GatewayConfig, GatewaySession, TcpZoneOwnerRpcTransport, ZoneId, ZoneTopology};
use mir2_protocol::{ChatType, ClientPacket, MirClass, MirGender, ServerPacket};
use mir2_simulation::{GroundDropLootSnapshot, WorldEntityKind};
use serde::Serialize;

const IMPORTED_MONSTER_NAME: &str = "Hen";
const FIXTURE_LEVEL: u16 = 255;
const FIXTURE_GOLD: u32 = 500;
const ATTACK_LIMIT: usize = 80;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Gate18GameplayEvidence {
    schema_version: u32,
    generated_at_ms: u64,
    run_id: String,
    gateway_process_id: u32,
    zone_host_endpoint: String,
    zone_host_id: String,
    zone_host_process_id: u32,
    first_account_id: String,
    second_account_id: String,
    first_character_index: i32,
    second_character_index: i32,
    starting_zone_id: String,
    handoff_zone_id: String,
    returned_zone_id: String,
    group_message_observed: bool,
    guild_message_observed: bool,
    death_observed: bool,
    revive_observed: bool,
    imported_monster_name: String,
    imported_monster_object_id: u32,
    attack_attempts: usize,
    monster_death_observed: bool,
    experience_before_kill: i64,
    experience_after_kill: i64,
    ledger_experience_after_kill: i64,
    dropped_item_key: String,
    dropped_item_quantity: u32,
    item_quantity_before_pickup: u32,
    item_quantity_after_pickup: u32,
    item_quantity_after_retry: u32,
    ledger_item_quantity_after_pickup: i64,
    ledger_item_quantity_after_retry: i64,
    assertions: BTreeMap<String, bool>,
    success: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = env::var("MIR2_ECONOMY_DATABASE_URL")
        .map_err(|_| "MIR2_ECONOMY_DATABASE_URL is required")?;
    let output = PathBuf::from(
        env::var("MIR2_GATE18_GAMEPLAY_OUT")
            .unwrap_or_else(|_| "docs/generated/regional/gate18-gameplay.json".to_string()),
    );
    let zone_host_endpoint = env::var("MIR2_ZONE_HOST_ADDR")
        .or_else(|_| env::var("MIR2_ZONE_HOST_ADDRS"))
        .map_err(|_| "MIR2_ZONE_HOST_ADDR or MIR2_ZONE_HOST_ADDRS is required")?;
    let generated_at_ms = now_ms();
    let run_id = format!("gate18-gameplay-{generated_at_ms}");
    let first_account_id = format!("g18ga{}", generated_at_ms % 1_000_000_000);
    let second_account_id = format!("g18gb{}", generated_at_ms % 1_000_000_000);

    let store = PostgresEconomyStore::new(database_url);
    store.ensure_migrated()?;
    let health_probe = TcpZoneOwnerRpcTransport::from_env(ZoneId::primary())
        .ok_or("MIR2_ZONE_HOST_ADDR or MIR2_ZONE_HOST_ADDRS is required")?;
    let host_before = health_probe.health()?;

    let topology = ZoneTopology::from_env()?;
    let registry = topology.zone_registry(default_zone_owner_lease_authority_from_env());
    let config = GatewayConfig::default().with_crystal_world_runtime();
    let (mut first, first_character_index) =
        start_session(&registry, config.clone(), &first_account_id, "RegionalA")?;
    let (mut second, second_character_index) =
        start_session(&registry, config, &second_account_id, "RegionalB")?;
    let starting_zone_id = first.zone_id().as_str().to_string();
    if second.zone_id() != first.zone_id() {
        return Err("gameplay peers did not start in the same map Zone".into());
    }

    apply_combat_fixture(&mut first)?;
    first.stage5_command("group.create", vec!["RegionalB".to_string()]);
    second.stage5_command("group.create", vec!["RegionalA".to_string()]);
    first.stage5_command("guild.create", vec!["RegionalGuild".to_string()]);
    second.stage5_command("guild.create", vec!["RegionalGuild".to_string()]);

    first.handle_packet(ClientPacket::Chat {
        message: "!!regional-group-sync".to_string(),
        linked_items: Vec::new(),
    });
    let group_packets = second.handle_packet(ClientPacket::KeepAlive { time: 18001 });
    let group_message_observed =
        chat_contains(&group_packets, ChatType::Group, "regional-group-sync");

    // Crystal applies a real two-second anti-spam interval to chat. Keep the
    // acceptance on that production path instead of bypassing the guard.
    thread::sleep(Duration::from_millis(2_100));
    first.handle_packet(ClientPacket::Chat {
        message: "!~regional-guild-sync".to_string(),
        linked_items: Vec::new(),
    });
    let guild_packets = second.handle_packet(ClientPacket::KeepAlive { time: 18002 });
    let guild_message_observed =
        chat_contains(&guild_packets, ChatType::Guild, "regional-guild-sync");

    let death_packets = first.handle_packet(ClientPacket::Chat {
        message: "@DIE".to_string(),
        linked_items: Vec::new(),
    });
    let death_observed = death_packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::Death { .. }))
        && death_packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::ObjectDied { .. }))
        && first.world_snapshot().player_hp == Some(0);
    let revive_packets = first.handle_packet(ClientPacket::TownRevive);
    let revive_observed = revive_packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::Revived))
        && revive_packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::ObjectRevived { .. }))
        && first.world_snapshot().player_hp.is_some_and(|hp| hp > 0);

    second.transfer_map("crystal:1:315:82");
    let handoff_zone_id = second.zone_id().as_str().to_string();
    let handoff_map = second.world_snapshot().map_file_name;
    second.transfer_map("crystal:0:330:270");
    let returned_zone_id = second.zone_id().as_str().to_string();
    let returned_map = second.world_snapshot().map_file_name;

    first.transfer_map("crystal:0:330:270");
    let existing_monster_ids = first
        .world_snapshot()
        .entities
        .into_iter()
        .filter(|entity| entity.kind == WorldEntityKind::Monster)
        .map(|entity| entity.object_id)
        .collect::<BTreeSet<_>>();
    first.stage5_command(
        "event.spawn",
        vec![IMPORTED_MONSTER_NAME.to_string(), "1".to_string()],
    );
    let monster = first
        .world_snapshot()
        .entities
        .into_iter()
        .find(|entity| {
            entity.kind == WorldEntityKind::Monster
                && entity.name == IMPORTED_MONSTER_NAME
                && !existing_monster_ids.contains(&entity.object_id)
        })
        .ok_or("imported Crystal monster fixture did not enter the shared Zone")?;
    let imported_monster_object_id = monster.object_id;
    let experience_before_kill = first.world_snapshot().player_experience;

    let mut attack_attempts = 0;
    let mut monster_death_observed = false;
    for attempt in 1..=ATTACK_LIMIT {
        let snapshot = first.world_snapshot();
        let Some(target) = snapshot
            .entities
            .into_iter()
            .find(|entity| entity.object_id == imported_monster_object_id)
        else {
            break;
        };
        if target.dead || target.hp.is_some_and(|hp| hp <= 0) {
            monster_death_observed = true;
            break;
        }
        let mut packets = first.attack(imported_monster_object_id);
        thread::sleep(Duration::from_millis(350));
        packets.extend(first.tick());
        thread::sleep(Duration::from_millis(350));
        attack_attempts = attempt;
        monster_death_observed |= packets.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::ObjectDied { info }
                    if info.object_id == imported_monster_object_id
            )
        });
        if monster_death_observed
            || first
                .world_snapshot()
                .ground_drops
                .iter()
                .any(|drop| drop.source_monster == IMPORTED_MONSTER_NAME)
        {
            monster_death_observed = true;
            break;
        }
    }

    // One extra command drains a kill award that may have been emitted by the
    // autonomous Zone cadence immediately after the final attack response.
    first.tick();
    let after_kill = first.world_snapshot();
    monster_death_observed |= after_kill.entities.iter().any(|entity| {
        entity.object_id == imported_monster_object_id
            && (entity.dead || entity.hp.is_some_and(|hp| hp <= 0))
    });
    let experience_after_kill = after_kill.player_experience;
    let experience_key = EconomyBalanceKey::experience(&first_account_id, first_character_index);
    let ledger_experience_after_kill = store.balance(&experience_key)?;
    let dropped_item = after_kill
        .ground_drops
        .into_iter()
        .find(|drop| {
            drop.source_monster == IMPORTED_MONSTER_NAME
                && matches!(drop.loot, GroundDropLootSnapshot::InventoryItem { .. })
        })
        .ok_or("imported Crystal monster produced no item drop")?;
    let dropped_item_key = match &dropped_item.loot {
        GroundDropLootSnapshot::InventoryItem { key, .. } => key.clone(),
        GroundDropLootSnapshot::Gold { .. } => unreachable!("item drop filtered above"),
    };
    let dropped_item_quantity = dropped_item.quantity;
    let item_quantity_before_pickup = item_quantity(&first, &dropped_item_key);
    first.transfer_map(&format!("crystal:0:{}:{}", dropped_item.x, dropped_item.y));
    let pickup_packets = first.handle_packet(ClientPacket::PickUp);
    let item_quantity_after_pickup = item_quantity(&first, &dropped_item_key);
    let item_key = EconomyBalanceKey::item_quantity(
        &first_account_id,
        first_character_index,
        &dropped_item_key,
    );
    let ledger_item_quantity_after_pickup = store.balance(&item_key)?;
    first.handle_packet(ClientPacket::PickUp);
    let item_quantity_after_retry = item_quantity(&first, &dropped_item_key);
    let ledger_item_quantity_after_retry = store.balance(&item_key)?;

    let host_after = health_probe.health()?;
    let assertions = BTreeMap::from([
        (
            "separateAuthenticatedZoneHostHandledBothPlayers".to_string(),
            !zone_host_endpoint.starts_with("127.0.0.1:")
                && !zone_host_endpoint.starts_with("localhost:")
                && host_after.host_id == host_before.host_id
                && host_after.session_count >= 2,
        ),
        (
            "groupStateAndChatWereShared".to_string(),
            group_message_observed,
        ),
        (
            "guildStateAndChatWereShared".to_string(),
            guild_message_observed,
        ),
        (
            "playerDeathAndTownReviveRoundTripped".to_string(),
            death_observed && revive_observed,
        ),
        (
            "mapHandoffReboundRemoteZone".to_string(),
            starting_zone_id == "map:0"
                && handoff_zone_id == "map:1"
                && handoff_map.as_deref() == Some("1")
                && returned_zone_id == "map:0"
                && returned_map.as_deref() == Some("0"),
        ),
        (
            "importedCrystalMonsterDiedThroughSharedCombat".to_string(),
            monster_death_observed && attack_attempts > 0,
        ),
        (
            "killExperienceCommittedToPostgres".to_string(),
            experience_after_kill > experience_before_kill
                && ledger_experience_after_kill == experience_after_kill,
        ),
        (
            "monsterItemDropPickedUpThroughPostgres".to_string(),
            pickup_packets
                .iter()
                .any(|packet| matches!(packet, ServerPacket::GainedItem { .. }))
                && item_quantity_after_pickup
                    == item_quantity_before_pickup.saturating_add(dropped_item_quantity)
                && ledger_item_quantity_after_pickup == i64::from(item_quantity_after_pickup),
        ),
        (
            "itemPickupRetryDidNotDuplicate".to_string(),
            item_quantity_after_retry == item_quantity_after_pickup
                && ledger_item_quantity_after_retry == ledger_item_quantity_after_pickup,
        ),
        (
            "monsterGroundObjectWasConsumed".to_string(),
            !first
                .world_snapshot()
                .ground_drops
                .iter()
                .any(|drop| drop.object_id == dropped_item.object_id),
        ),
    ]);
    let success = assertions.values().all(|passed| *passed);
    let evidence = Gate18GameplayEvidence {
        schema_version: 1,
        generated_at_ms,
        run_id,
        gateway_process_id: std::process::id(),
        zone_host_endpoint,
        zone_host_id: host_after.host_id,
        zone_host_process_id: host_after.process_id,
        first_account_id,
        second_account_id,
        first_character_index,
        second_character_index,
        starting_zone_id,
        handoff_zone_id,
        returned_zone_id,
        group_message_observed,
        guild_message_observed,
        death_observed,
        revive_observed,
        imported_monster_name: IMPORTED_MONSTER_NAME.to_string(),
        imported_monster_object_id,
        attack_attempts,
        monster_death_observed,
        experience_before_kill,
        experience_after_kill,
        ledger_experience_after_kill,
        dropped_item_key,
        dropped_item_quantity,
        item_quantity_before_pickup,
        item_quantity_after_pickup,
        item_quantity_after_retry,
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
        return Err("Gate 18 remote gameplay acceptance failed".into());
    }
    Ok(())
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

fn apply_combat_fixture(session: &mut GatewaySession) -> Result<(), Box<dyn std::error::Error>> {
    let snapshot = session.world_snapshot();
    let identity = session
        .active_identity()
        .ok_or("combat fixture requires an active identity")?;
    let player = snapshot
        .entities
        .iter()
        .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
        .ok_or("combat fixture requires an in-world player")?;
    let state = serde_json::json!({
        "character": {
            "name": identity.character_name,
            "level": FIXTURE_LEVEL,
            "class": "Warrior",
            "gender": "Male"
        },
        "mapFileName": snapshot.map_file_name.unwrap_or_else(|| "0".to_string()),
        "mapTitle": snapshot.map_title.unwrap_or_else(|| "BichonProvince".to_string()),
        "position": { "x": player.x, "y": player.y },
        "direction": player.direction,
        "hp": 10000,
        "maxHp": 10000,
        "mp": 10000,
        "maxMp": 10000,
        "experience": 0,
        "maxExperience": 1,
        "gold": FIXTURE_GOLD,
        "credit": 0,
        "inventoryItemsJson": [],
        "beltItemsJson": [],
        "storageItemsJson": [],
        "equipmentItemsJson": []
    });
    session.stage5_command("qa.applyNativeState", vec![state.to_string()]);
    let applied = session.world_snapshot();
    if applied.gold != FIXTURE_GOLD || applied.player_hp != Some(10000) {
        return Err("remote QA combat fixture was not applied".into());
    }
    Ok(())
}

fn chat_contains(packets: &[ServerPacket], expected: ChatType, needle: &str) -> bool {
    packets.iter().any(|packet| match packet {
        ServerPacket::Chat { message, chat_type } => {
            *chat_type == expected && message.contains(needle)
        }
        ServerPacket::ObjectChat {
            text, chat_type, ..
        } => *chat_type == expected && text.contains(needle),
        _ => false,
    })
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

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}
