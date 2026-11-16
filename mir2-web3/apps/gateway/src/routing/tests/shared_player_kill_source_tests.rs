//! Prepared ordinary players/equipment, then the real Gateway Attack reducer
//! issues a causal death. The test pauses exactly before the private consumer
//! to exercise publication/ACK crash windows; no PK receipt is manufactured.
//! This is isolated in-process Gateway evidence, not socket/native acceptance.
use super::*;
use mir2_simulation::{
    AccountRecord, AccountStore, CharacterRecord, ZoneOwnedPetPlayerKillReceipt,
};
use serde_json::json;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const MAP: &str = "pk-source-boundary";
const OWNER: &str = "pk-source-owner";
const VICTIM: &str = "pk-source-victim";

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Self(std::env::temp_dir().join(format!("mir2-pk-source-{}-{nonce}", std::process::id())))
    }
    fn path(&self) -> PathBuf {
        self.0.join("accounts.json")
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if self.0.is_absolute() && self.0.parent() == Some(std::env::temp_dir().as_path()) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}
fn scene(directory: &Directory) -> GatewayConfig {
    let mut config = GatewayConfig::default()
        .with_crystal_world_runtime()
        .with_account_store_path(directory.path());
    config.monster_spawn_source = mir2_simulation::MonsterSpawnSource::StarterScenario;
    config.map.file_name = MAP.into();
    config.map.title = "PK source boundary fixture".into();
    config.spawn = Point { x: 10, y: 10 };
    config.map_collision.map_file_name = MAP.into();
    config.map_collision.map_width = 40;
    config.map_collision.map_height = 40;
    config.map_collision.play_bounds = mir2_game_data::MapBounds {
        min_x: 0,
        max_x: 39,
        min_y: 0,
        max_y: 39,
    };
    config.map_collision.region_bounds = config.map_collision.play_bounds.clone();
    config.map_collision.blocked_cells.clear();
    config.map_collision.doors.clear();
    config.visible_monsters.clear();
    config.visible_players.clear();
    config.visible_npcs.clear();
    config.safe_zones.clear();
    config.map_transfers.clear();
    config
}
fn prepared(directory: &Directory) -> GatewayConfig {
    let config = scene(directory);
    let equipment = [
        ("BloodStealerSword", mir2_simulation::EquipmentSlot::Weapon),
        ("DragonPendant5", mir2_simulation::EquipmentSlot::Necklace),
        ("SharpBracelet", mir2_simulation::EquipmentSlot::BraceletLeft),
        ("SurvivalBracelet", mir2_simulation::EquipmentSlot::BraceletRight),
    ].into_iter().map(|(name, slot)| {
        let item = mir2_game_data::crystal_item_by_name(name).unwrap();
        json!({"key": format!("crystal-item-{}", item.item_index), "name": item.name,
            "slot": slot, "quantity": 1, "icon": item.image, "shape": item.shape,
            "description": "prepared canonical equipment", "durability_current": item.durability.max(1),
            "durability_max": item.durability.max(1), "attack": 0, "defence": 0}).to_string()
    }).collect::<Vec<_>>();
    let mut store = config.account_store.lock().unwrap();
    store.accounts.clear();
    for (account, index, name, level, y) in [
        (OWNER, 1, "SourceAttacker", 40, 10),
        (VICTIM, 2, "SourceVictim", 1, 9),
    ] {
        let mut record = AccountRecord::new(CharacterRecord {
            index,
            name: name.into(),
            level,
            class: MirClass::Warrior,
            gender: MirGender::Male,
        });
        record.password = "isolated-pk-source-password".into();
        let save = record.saves.get_mut(&index).unwrap();
        save.map_file_name = MAP.into();
        save.map_title = config.map.title.clone();
        save.position = Point { x: 10, y };
        if account == OWNER {
            save.equipment_items_json = equipment.clone();
        }
        store.accounts.insert(account.into(), record);
    }
    drop(store);
    config.save_account_store().unwrap();
    config
}
fn login(
    runtime: &mut SharedInProcessZoneSessionRuntime,
    account: &str,
    index: i32,
) -> Vec<ServerPacket> {
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: account.into(),
            password: "isolated-pk-source-password".into(),
        }))
        .unwrap();
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index: index,
        }))
        .unwrap()
}
fn runtime(
    state: Arc<Mutex<SharedInProcessZoneState>>,
    config: GatewayConfig,
) -> SharedInProcessZoneSessionRuntime {
    let mut runtime = shared_session_runtime(state);
    runtime.inner = InProcessWorldRuntime::new(config);
    runtime
}
fn actual_death(
    config: &GatewayConfig,
) -> (
    SharedInProcessZoneSessionRuntime,
    SharedInProcessZoneSessionRuntime,
    ZoneOwnedPetPlayerKillReceipt,
) {
    let state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut owner = runtime(state.clone(), config.clone());
    let mut victim = runtime(state.clone(), config.clone());
    login(&mut owner, OWNER, 1);
    login(&mut victim, VICTIM, 2);
    owner
        .execute(WorldCommand::ClientPacket(ClientPacket::ChangeAMode {
            mode: 5,
        }))
        .unwrap();
    // Prepared health makes the boundary deterministic. Damage/accuracy still
    // come from the authenticated owner's actual canonical equipment/stats.
    victim
        .inner
        .force_authoritative_player_vitals_with_max_hp(Some(1), Some(1), None);
    victim.sync_zone_snapshot();
    let attacker = owner.inner.zone_player_combat_stats();
    let defender = victim.inner.zone_player_combat_stats();
    assert!(attacker.min_dc > defender.max_ac);
    assert!(attacker.accuracy >= defender.agility);
    let command = WorldCommand::ClientPacket(ClientPacket::Attack {
        direction: MirDirection::Up,
        spell: Spell::None,
    });
    let attack = owner
        .prepare_zone_native_player_attack(&command)
        .expect("normal Attack resolves the real adjacent Human");
    assert!(attack.is_player_target);
    // Same normal reducer as execute(), paused before its tail consumes PK.
    let output = owner.execute_zone_native_player_attack(attack);
    let victim_id = victim.current_zone_player_object_id().unwrap();
    assert!(output.iter().any(
        |packet| matches!(packet, ServerPacket::ObjectDied { info } if info.object_id == victim_id)
    ));
    let key = owner.current_presence_key().unwrap();
    let state = state.lock().unwrap();
    let receipts = state.pending_zone_player_kills.get(&key).unwrap();
    assert_eq!(receipts.len(), 1);
    let receipt = receipts[0].clone();
    assert!(receipt.direct_player && receipt.unlawful && !receipt.protected_by_law);
    let active = owner.inner.active_identity().unwrap();
    assert_eq!(receipt.owner_account_id, active.account_id);
    assert_eq!(receipt.owner_character_index, active.character_index);
    assert!(
        !receipt.owner_online_identity.is_empty()
            && receipt.owner_object_id != 0
            && receipt.sequence != 0
    );
    assert_eq!((receipt.pet_object_id, receipt.pet_incarnation), (0, 0));
    assert_eq!(
        Some(receipt.owner_life_generation),
        state
            .zone_manager
            .player_life_generation(&receipt.owner_session_id),
        "the receipt carries the actual current Human life, including its initial zero generation"
    );
    assert!(state
        .zone_manager
        .issued_owned_pet_kill_is_current(&receipt));
    drop(state);
    (owner, victim, receipt)
}
fn stored(config: &GatewayConfig) -> mir2_simulation::CharacterSaveRecord {
    config.account_store.lock().unwrap().accounts[OWNER].saves[&1].clone()
}
fn on_disk(directory: &Directory) -> mir2_simulation::CharacterSaveRecord {
    let store: AccountStore =
        serde_json::from_slice(&std::fs::read(directory.path()).unwrap()).unwrap();
    store.accounts[OWNER].saves[&1].clone()
}
fn checkpoint(owner: &SharedInProcessZoneSessionRuntime) -> Vec<u8> {
    serde_json::to_vec(&owner.zone_state.lock().unwrap().world_checkpoint().unwrap()).unwrap()
}
fn restored(bytes: &[u8]) -> Arc<Mutex<SharedInProcessZoneState>> {
    let checkpoint = serde_json::from_slice(bytes).unwrap();
    Arc::new(Mutex::new(
        SharedInProcessZoneState::restore(checkpoint).unwrap(),
    ))
}
fn cold_source(
    directory: &Directory,
    owner: SharedInProcessZoneSessionRuntime,
    victim: SharedInProcessZoneSessionRuntime,
    config: GatewayConfig,
) -> GatewayConfig {
    let old = Arc::downgrade(&config.account_store);
    drop(owner);
    drop(victim);
    drop(config);
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while old.upgrade().is_some() {
        assert!(
            std::time::Instant::now() < deadline,
            "cold boundary releases every old Source/store owner"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let next = scene(directory);
    assert!(
        serde_json::to_value(stored(&next)).unwrap()
            == serde_json::to_value(on_disk(directory)).unwrap(),
        "fresh Source loads the actual disk record after the old file authority is gone"
    );
    next
}
fn weapon_luck(save: &mir2_simulation::CharacterSaveRecord) -> i64 {
    save.equipment_items_json
        .iter()
        .map(|item| serde_json::from_str::<serde_json::Value>(item).unwrap())
        .find(|item| item["slot"] == "weapon")
        .unwrap()["added_luck"]
        .as_i64()
        .unwrap_or(0)
}

#[test]
fn actual_player_death_known_source_failure_cold_checkpoint_retry_publishes_then_acknowledges_once()
{
    let directory = Directory::new();
    let config = prepared(&directory);
    let (mut owner, victim, receipt) = actual_death(&config);
    let before = stored(&config);
    let private_before = owner.inner.active_character_checkpoint().unwrap();
    config.inject_account_store_transaction_fault(
        mir2_simulation::AccountStoreTransactionFault::BeforePersist,
    );
    assert!(owner.consume_pending_player_kills().is_err());
    assert!(
        serde_json::to_value(stored(&config)).unwrap() == serde_json::to_value(&before).unwrap(),
        "known PK publication failure restores every durable field"
    );
    assert!(
        serde_json::to_value(owner.inner.active_character_checkpoint().unwrap()).unwrap()
            == serde_json::to_value(&private_before).unwrap(),
        "known PK publication failure restores the complete private checkpoint"
    );
    let key = owner.current_presence_key().unwrap();
    assert_eq!(
        owner.zone_state.lock().unwrap().pending_zone_player_kills[&key],
        vec![receipt.clone()]
    );
    let bytes = checkpoint(&owner);
    let recovered_state = restored(&bytes);
    assert!(recovered_state
        .lock()
        .unwrap()
        .zone_manager
        .issued_owned_pet_kill_is_current(&receipt));
    let next_config = cold_source(&directory, owner, victim, config);
    let mut next = runtime(recovered_state.clone(), next_config.clone());
    let packets = login(&mut next, OWNER, 1);
    assert_eq!(next.inner.world_snapshot().player_pk_points, 100);
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::ColourChanged { .. })));
    let durable = on_disk(&directory);
    assert_eq!(durable.pk_points, 100);
    assert_eq!(durable.player_kill_receipts.len(), 1);
    assert!(!recovered_state
        .lock()
        .unwrap()
        .zone_manager
        .issued_owned_pet_kill_is_current(&receipt));
    assert!(!recovered_state
        .lock()
        .unwrap()
        .pending_zone_player_kills
        .contains_key(&key));
    assert!(next.consume_pending_player_kills().unwrap().is_empty());
    let after = stored(&next_config);
    assert!(serde_json::to_value(after).unwrap() == serde_json::to_value(durable).unwrap());
}

#[test]
fn actual_player_death_committed_before_ack_cold_replay_keeps_penalty_and_weapon_exactly_once() {
    let directory = Directory::new();
    let config = prepared(&directory);
    let (mut owner, victim, receipt) = actual_death(&config);
    let original_luck = weapon_luck(&stored(&config));
    // Inject a process boundary after the real source publishes, immediately
    // before Gateway acknowledges the exact still-issued causal receipt.
    let source_packets = owner
        .inner
        .apply_shared_owned_pet_player_kill(&receipt)
        .unwrap();
    assert!(source_packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::ColourChanged { .. })));
    let committed = on_disk(&directory);
    assert_eq!(committed.pk_points, 100);
    assert_eq!(committed.player_kill_receipts.len(), 1);
    assert_eq!(
        weapon_luck(&committed),
        original_luck - i64::from(receipt.curse_roll == 0),
        "the real issuer's sampled curse decision changes luck exactly once"
    );
    assert_eq!(
        source_packets
            .iter()
            .filter(|packet| matches!(packet, ServerPacket::RefreshItem { .. }))
            .count(),
        usize::from(receipt.curse_roll == 0)
    );
    assert!(
        serde_json::to_value(stored(&config)).unwrap() == serde_json::to_value(&committed).unwrap()
    );
    let bytes = checkpoint(&owner);
    let recovered_state = restored(&bytes);
    let next_config = cold_source(&directory, owner, victim, config);
    let mut next = runtime(recovered_state.clone(), next_config.clone());
    let packets = login(&mut next, OWNER, 1);
    let after = stored(&next_config);
    assert_eq!(after.pk_points, committed.pk_points);
    assert_eq!(
        after.equipment_items_json, committed.equipment_items_json,
        "any real source curse is retained without a second decrement"
    );
    assert_eq!(after.player_kill_receipts, committed.player_kill_receipts);
    assert_eq!(
        packets
            .iter()
            .filter(|packet| matches!(packet, ServerPacket::RefreshItem { .. }))
            .count(),
        0
    );
    assert!(!recovered_state
        .lock()
        .unwrap()
        .zone_manager
        .issued_owned_pet_kill_is_current(&receipt));
    assert!(next.consume_pending_player_kills().unwrap().is_empty());
}

#[test]
fn actual_causal_player_kill_tampering_is_rejected_before_source_and_original_remains_retriable() {
    let directory = Directory::new();
    let config = prepared(&directory);
    let (mut owner, _victim, receipt) = actual_death(&config);
    let key = owner.current_presence_key().unwrap();
    let before = stored(&config);
    let mut altered = receipt.clone();
    altered.curse_roll = (altered.curse_roll + 1) % 4;
    owner
        .zone_state
        .lock()
        .unwrap()
        .pending_zone_player_kills
        .insert(key.clone(), vec![altered]);
    assert!(owner.consume_pending_player_kills().is_err());
    assert!(
        serde_json::to_value(stored(&config)).unwrap() == serde_json::to_value(before).unwrap()
    );
    assert!(owner
        .zone_state
        .lock()
        .unwrap()
        .zone_manager
        .issued_owned_pet_kill_is_current(&receipt));
    owner
        .zone_state
        .lock()
        .unwrap()
        .pending_zone_player_kills
        .insert(key, vec![receipt]);
    let output = owner.consume_pending_player_kills().unwrap();
    assert!(output
        .iter()
        .any(|packet| matches!(packet, ServerPacket::ColourChanged { .. })));
    assert_eq!(stored(&config).pk_points, 100);
    assert_eq!(stored(&config).player_kill_receipts.len(), 1);
    assert!(owner.consume_pending_player_kills().unwrap().is_empty());
}

#[test]
fn actual_summoned_pet_committed_source_keeps_pending_growth_during_gateway_mapping_outage() {
    let directory = Directory::new();
    let config = prepared(&directory);
    let amulet = mir2_game_data::crystal_item_by_name("Amulet").unwrap();
    let mut record = AccountRecord::new(CharacterRecord {
        index: 1,
        name: "SourceTaoist".into(),
        level: 40,
        class: MirClass::Taoist,
        gender: MirGender::Male,
    });
    record.password = "isolated-pk-source-password".into();
    let save = record.saves.get_mut(&1).unwrap();
    save.map_file_name = MAP.into();
    save.map_title = config.map.title.clone();
    save.position = Point { x: 10, y: 10 };
    save.mp = 200;
    save.max_mp = 200;
    // The custom fixture has no content profile; prepare the original level40
    // ExpList threshold instead of CharacterSaveRecord's legacy default100.
    save.max_experience = 12_000_000;
    save.skill_states_json = vec![json!({"key":"summonskeleton","name":"SummonSkeleton",
        "description":"prepared learned magic","level":0,"experience":0,"cooldown_ticks":0,"cooldown_ends_at":0}).to_string()];
    save.equipment_items_json = vec![json!({"key":format!("crystal-item-{}",amulet.item_index),
        "name":amulet.name,"slot":mir2_simulation::EquipmentSlot::Amulet,"quantity":20,"icon":amulet.image,
        "shape":amulet.shape,"description":"prepared canonical Amulet","durability_current":amulet.durability.max(1),
        "durability_max":amulet.durability.max(1),"attack":0,"defence":0}).to_string()];
    assert!(save.saved_pets.is_empty(), "no prepared pet record");
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .insert(OWNER.into(), record);
    config.save_account_store().unwrap();
    let state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut owner = runtime(state.clone(), config.clone());
    login(&mut owner, OWNER, 1);
    let local = owner.local_self_object_id().unwrap();
    let cast = owner
        .execute(WorldCommand::ClientPacket(ClientPacket::Magic {
            object_id: local,
            spell: Spell::SummonSkeleton,
            direction: MirDirection::Right,
            target_id: 0,
            location: Point { x: 11, y: 10 },
            spell_target_lock: false,
        }))
        .unwrap();
    assert!(cast.iter().any(|packet| matches!(
        packet,
        ServerPacket::Magic {
            spell: Spell::SummonSkeleton,
            cast: true,
            ..
        }
    )));
    std::thread::sleep(Duration::from_millis(1_900));
    owner.dispatch_zone_player_command(
        ZoneCommand::Tick {
            now_ms: SharedInProcessZoneSessionRuntime::zone_now_ms(),
        },
        false,
    );
    let sid = owner.current_zone_session_id().unwrap();
    let initial = state
        .lock()
        .unwrap()
        .zone_manager
        .capture_player_saved_pets(&sid, SharedInProcessZoneSessionRuntime::zone_now_ms());
    let initial_json = serde_json::to_value(initial.for_durable_storage()).unwrap();
    assert_eq!(
        initial_json["pets"].as_array().unwrap().len(),
        1,
        "actual normal delayed summon produces a pet"
    );
    owner.refresh_pet_progress_context().unwrap();
    owner.refresh_shared_experience_profiles().unwrap();
    let (target, direction) = {
        let state = state.lock().unwrap();
        let origin = state.zone_manager.player_transform(&sid).unwrap().0;
        let occupied = state
            .zone_manager
            .native_monster_snapshots(&ZoneKey::for_map(MAP));
        [
            (MirDirection::Up, 0, -1),
            (MirDirection::Right, 1, 0),
            (MirDirection::Left, -1, 0),
            (MirDirection::Down, 0, 1),
        ]
        .into_iter()
        .find_map(|(direction, dx, dy)| {
            let point = Point {
                x: origin.x + dx,
                y: origin.y + dy,
            };
            (!occupied
                .iter()
                .any(|monster| !monster.dead && monster.position == point))
            .then_some((point, direction))
        })
        .unwrap()
    };
    owner.dispatch_zone_player_command(
        ZoneCommand::SpawnMonster {
            session_id: sid.clone(),
            now_ms: SharedInProcessZoneSessionRuntime::zone_now_ms(),
            monster: ZoneMonsterSpawn {
                crystal_drop_seed: None,
                object_id: 930_001,
                name: "Scarecrow".into(),
                name_colour_argb: -1,
                image: 0,
                ai: 0,
                disposition: Some(WorldEntityDisposition::Hostile),
                level: 40,
                hp: 1,
                max_hp: 1,
                experience: 20_000,
                move_speed_ms: 60_000,
                attack_speed_ms: 60_000,
                friendly_guild: None,
                position: target.clone(),
                direction: MirDirection::Down,
                defense: Default::default(),
                respawn: None,
                drops: vec![],
            },
        },
        false,
    );
    let command = WorldCommand::ClientPacket(ClientPacket::Attack {
        direction,
        spell: Spell::None,
    });
    let attack = owner.prepare_zone_native_player_attack(&command).unwrap();
    assert_eq!(
        attack.object_id, 930_001,
        "normal Attack selects the prepared encounter's actual birth cell"
    );
    let mut death = owner.execute_zone_native_player_attack(attack);
    // Ordinary melee queues its immediate impact. Use the existing normal
    // issuer/collection boundary so this test can pause after the cadence Tick
    // issues its proof and before the private Source/pet publisher consumes it.
    let (tick_packets, claims, awards, damages, heals) = owner
        .dispatch_zone_player_command_collecting_claims(
            ZoneCommand::Tick {
                now_ms: SharedInProcessZoneSessionRuntime::zone_now_ms(),
            },
            false,
        );
    death.extend(tick_packets);
    assert!(claims.is_empty() && damages.is_empty() && heals.is_empty());
    let remaining = state
        .lock()
        .unwrap()
        .zone_manager
        .native_monster_snapshots(&ZoneKey::for_map(MAP))
        .into_iter()
        .find(|monster| monster.object_id == 930_001)
        .map(|monster| (monster.position, monster.hp, monster.max_hp));
    assert!(death.iter().any(
        |packet| matches!(packet, ServerPacket::ObjectDied { info } if info.object_id == 930_001)
    ),"ordinary actual source kill; response kinds {:?}; remaining {:?}; attacker {:?}",
        death.iter().map(mir2_protocol::server_packet_name).collect::<Vec<_>>(),
        remaining,
        owner.inner.zone_player_combat_stats());
    let key = owner.current_presence_key().unwrap();
    assert_eq!(awards.len(), 1);
    let award = awards[0].clone();
    assert!(state
        .lock()
        .unwrap()
        .owned_monster_kill_proof_is_current(&key, &award));
    let outcome = owner.commit_account_inventory_outcome(SharedAccountInventoryCommandEnvelope {
        identity: owner.inner.active_identity().unwrap(),
        command: SharedAccountInventoryCommand::MonsterKillAward(award.clone()),
    });
    assert!(
        matches!(outcome, SharedAccountInventoryCommitOutcome::Confirmed(ref receipt) if receipt.committed)
    );
    state
        .lock()
        .unwrap()
        .forget_owned_monster_kill_proof(&key, &award);
    let committed = owner.inner.pending_shared_pet_experience_commit().unwrap();
    assert_eq!(committed.1, vec![20_000]);
    let durable = on_disk(&directory);
    let durable_json = serde_json::to_value(durable.saved_pets.for_durable_storage()).unwrap();
    assert_eq!(durable_json["pets"][0]["level"], 1);
    assert_eq!(durable_json["pets"][0]["experience"], 40_000);
    let mapping = state.lock().unwrap().zone_sessions.remove(&key).unwrap();
    assert!(owner.publish_committed_pet_progress().is_err());
    let pending = owner.inner.pending_shared_pet_experience_commit().unwrap();
    assert_eq!(
        pending.1, committed.1,
        "the exact already-saved earned steps survive temporary publication admission loss"
    );
    assert!(pending.0.before_snapshot() == committed.0.before_snapshot());
    state
        .lock()
        .unwrap()
        .zone_sessions
        .insert(key.clone(), mapping);
    owner.publish_committed_pet_progress().unwrap();
    assert!(owner.inner.pending_shared_pet_experience_commit().is_none());
    let mirrored = state
        .lock()
        .unwrap()
        .zone_manager
        .capture_player_saved_pets(&sid, SharedInProcessZoneSessionRuntime::zone_now_ms());
    assert!(mirrored.for_durable_storage() == durable.saved_pets.for_durable_storage());
    let queued = state
        .lock()
        .unwrap()
        .pending_zone_packets
        .get(&key)
        .cloned()
        .unwrap_or_default();
    assert!(queued
        .iter()
        .any(|packet| matches!(packet, ServerPacket::ObjectColourChanged { .. })));
    owner.publish_committed_pet_progress().unwrap();
    assert_eq!(
        state
            .lock()
            .unwrap()
            .pending_zone_packets
            .get(&key)
            .cloned()
            .unwrap_or_default(),
        queued,
        "a repeated publication is quiet after the exact earned steps are acknowledged"
    );
}
