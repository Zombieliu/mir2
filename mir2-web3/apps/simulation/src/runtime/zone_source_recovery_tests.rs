//! Pure runtime/cache proofs. The test-only stage cannot claim that PostgreSQL
//! has validated a lease or that the Host published its recovery marker.
use super::*;
use crate::{AccountStoreDatabaseMode, ZoneSaveFence};
use mir2_protocol::{ClientPacket, MirDirection, Point};
use std::collections::BTreeMap;

fn setup() -> (SimulationConfig, SimulationSession) {
    let base = SimulationConfig::default();
    let mut original = SimulationSession::new(base.clone());
    original.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    original.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let checkpoint = original.active_character_checkpoint().unwrap();
    base.account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut("demo")
        .unwrap()
        .saves
        .insert(0, checkpoint.clone());
    let mut replica = SimulationSession::new(base.fork_for_replica_apply().unwrap());
    replica
        .restore_replica_session_checkpoint(&original.active_identity().unwrap(), &checkpoint)
        .unwrap();
    let mut authoritative = base;
    authoritative.account_store_database_mode = AccountStoreDatabaseMode::SourceOfTruth;
    authoritative.account_store_database_url =
        Some("postgresql://unused.invalid/canonical-recovery".into());
    authoritative = authoritative
        .with_zone_save_fence(
            ZoneSaveFence::new("logical-owner-group".into(), "successor-owner".into(), 19).unwrap(),
        )
        .unwrap();
    (authoritative, replica)
}
fn stage(config: &SimulationConfig, save: CharacterSaveRecord) -> PreparedZoneSourceRecovery {
    let mut accounts = config.account_store.lock().unwrap().accounts.clone();
    let account = accounts.get_mut("demo").unwrap();
    account
        .characters
        .retain(|character| character.index != save.character.index);
    account.characters.push(save.character.clone());
    account.saves.insert(save.character.index, save);
    config.zone_source_recovery_fixture(accounts, 8).unwrap()
}
fn runtime_config(session: &SimulationSession) -> SimulationConfig {
    session
        .app
        .world()
        .resource::<RuntimeConfigResource>()
        .config
        .clone()
}

#[test]
fn zone_source_recovery_installs_newer_assets_but_preserves_shared_transform_vitals_and_clock() {
    let (config, mut replica) = setup();
    let mut canonical = replica.active_character_checkpoint().unwrap();
    canonical.revision += 2;
    canonical.gold = 317;
    let source = stage(&config, canonical.clone());
    config.install_zone_source_recovery_cache(&source).unwrap();
    let position = Point {
        x: canonical.position.x + 1,
        y: canonical.position.y,
    };
    let vitals = super::super::components::PlayerVitals {
        hp: 23,
        max_hp: 321,
        mp: 49,
        max_mp: 456,
    };
    {
        let world = replica.app.world_mut();
        let player = player_entity(world).unwrap();
        world.entity_mut(player).insert((
            Position(position.clone()),
            Facing(MirDirection::Left),
            vitals,
        ));
        let mut runtime = world.resource_mut::<PlayerRuntimeResource>();
        runtime.player_position = position.clone();
        runtime.player_direction = MirDirection::Left;
        runtime.player_vitals = vitals;
        runtime.player_dead = true;
        runtime.last_damaged_tick = 88;
        runtime.last_regen_tick = 91;
        runtime.chat_next_allowed_at_ms = 41_123;
        drop(runtime);
        super::super::resources::set_runtime_tick(world, 100);
        super::super::resources::mark_crystal_packet_action(
            world,
            super::super::resources::PlayerActionKind::Attack,
            3,
        );
        super::super::resources::mark_crystal_player_move(world, false);
    }
    let timing = format!(
        "{:?}/{:?}",
        replica.app.world().resource::<PlayerActionTimingResource>(),
        replica
            .app
            .world()
            .resource::<PlayerMovementTimingResource>()
    );
    replica
        .install_zone_source_recovery(&source, &config)
        .unwrap();
    let recovered = replica.active_character_checkpoint().unwrap();
    assert_eq!(
        (recovered.gold, recovered.revision),
        (317, canonical.revision)
    );
    let world = replica.app.world();
    let player = player_entity(world).unwrap();
    assert_eq!(entity_position(world, player).unwrap(), position);
    assert_eq!(entity_facing(world, player), Some(MirDirection::Left));
    let observed = entity_player_vitals(world, player).unwrap();
    assert_eq!(
        (observed.hp, observed.max_hp, observed.mp, observed.max_mp),
        (23, 321, 49, 456)
    );
    assert!(current_player_is_dead(world));
    assert_eq!(super::super::resources::runtime_tick(world), 100);
    assert_eq!(
        format!(
            "{:?}/{:?}",
            world.resource::<PlayerActionTimingResource>(),
            world.resource::<PlayerMovementTimingResource>()
        ),
        timing
    );
    let runtime = world.resource::<PlayerRuntimeResource>();
    assert_eq!(
        (
            runtime.last_damaged_tick,
            runtime.last_regen_tick,
            runtime.chat_next_allowed_at_ms
        ),
        (88, 91, 41_123)
    );
    let rebound = runtime_config(&replica);
    source.validate_source_binding(&rebound).unwrap();
    assert_eq!(rebound.zone_save_fence(), Some(source.fence()));
    assert!(!rebound.has_isolated_replica_bootstrap_authority());
    assert_eq!(
        config.account_store.lock().unwrap().accounts["demo"].saves[&0].position,
        canonical.position
    );
}

#[test]
fn zone_source_recovery_preserves_same_session_skill_buff_queue_and_potion_deadlines() {
    let (config, mut replica) = setup();
    {
        let world = replica.app.world_mut();
        let mut skill = super::super::skills::seed_skills().remove(0);
        skill.cooldown_ends_at = 177;
        skill.cast_time_ms = 912;
        world.resource_mut::<SkillResource>().skills = vec![skill];
        world.resource_mut::<BuffResource>().buffs = vec![super::super::buffs::BuffState {
            key: "recovery-test".into(),
            name: "test".into(),
            description: "test".into(),
            expires_at_tick: 171,
            real_time_duration: None,
            attack_bonus: 0,
            defence_bonus: 0,
            stats: vec![],
        }];
        world
            .resource_mut::<PotionRecoveryResource>()
            .pending_pot_health_amount = 17;
        world
            .resource_mut::<RuntimeQueueResource>()
            .pending_movement_command = Some(super::super::resources::PendingMovementCommand {
            direction: MirDirection::Right,
            running: false,
        });
        super::super::resources::set_runtime_tick(world, 100);
    }
    let mut canonical = replica.active_character_checkpoint().unwrap();
    canonical.revision += 1;
    canonical.gold = 318;
    let source = stage(&config, canonical);
    config.install_zone_source_recovery_cache(&source).unwrap();
    replica
        .install_zone_source_recovery(&source, &config)
        .unwrap();
    let world = replica.app.world();
    let skill = &world.resource::<SkillResource>().skills[0];
    assert_eq!((skill.cooldown_ends_at, skill.cast_time_ms), (177, 912));
    assert_eq!(
        world.resource::<BuffResource>().buffs[0].expires_at_tick,
        171
    );
    assert_eq!(
        world
            .resource::<PotionRecoveryResource>()
            .pending_pot_health_amount,
        17
    );
    assert_eq!(
        world
            .resource::<RuntimeQueueResource>()
            .pending_movement_command
            .unwrap()
            .direction,
        MirDirection::Right
    );
    assert_eq!(super::super::resources::runtime_tick(world), 100);
}

#[test]
fn zone_source_recovery_rejects_equal_revision_economic_conflicts_before_runtime_mutation() {
    let (config, mut replica) = setup();
    let before = replica.active_character_checkpoint().unwrap();
    let mut canonical = before.clone();
    canonical.gold += 1;
    let source = stage(&config, canonical);
    config.install_zone_source_recovery_cache(&source).unwrap();
    let error = replica
        .install_zone_source_recovery(&source, &config)
        .unwrap_err();
    assert!(
        error.contains("equal-revision economic state conflict"),
        "{error}"
    );
    assert_eq!(
        replica.active_character_checkpoint().unwrap().gold,
        before.gold
    );
    assert!(runtime_config(&replica).has_isolated_replica_bootstrap_authority());
}

#[test]
fn zone_source_recovery_rejects_older_private_revision_and_different_shared_map() {
    let (config, mut replica) = setup();
    let mut canonical = replica.active_character_checkpoint().unwrap();
    let revision = canonical.revision;
    replica
        .app
        .world()
        .resource::<SessionResource>()
        .bind_active_save_revision(revision + 1);
    let source = stage(&config, canonical);
    config.install_zone_source_recovery_cache(&source).unwrap();
    assert!(replica
        .install_zone_source_recovery(&source, &config)
        .unwrap_err()
        .contains("newer private revision"));

    let (config, mut replica) = setup();
    canonical = replica.active_character_checkpoint().unwrap();
    canonical.revision += 1;
    canonical.map_file_name = "another-canonical-map".into();
    let source = stage(&config, canonical);
    config.install_zone_source_recovery_cache(&source).unwrap();
    assert!(replica
        .install_zone_source_recovery(&source, &config)
        .unwrap_err()
        .contains("current shared map"));
}

#[test]
fn zone_source_recovery_rejects_stale_cache_wrong_actor_and_untrusted_runtime() {
    let (config, mut replica) = setup();
    let source = stage(&config, replica.active_character_checkpoint().unwrap());
    assert!(replica
        .install_zone_source_recovery(&source, &config)
        .is_err());
    config.install_zone_source_recovery_cache(&source).unwrap();
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut("demo")
        .unwrap()
        .saves
        .get_mut(&0)
        .unwrap()
        .revision += 1;
    assert!(replica
        .install_zone_source_recovery(&source, &config)
        .is_err());

    let (config, mut replica) = setup();
    let source = config
        .zone_source_recovery_fixture(BTreeMap::new(), 8)
        .unwrap();
    config.install_zone_source_recovery_cache(&source).unwrap();
    assert!(replica
        .install_zone_source_recovery(&source, &config)
        .unwrap_err()
        .contains("outside its canonical read set"));

    let (config, replica) = setup();
    let source = stage(&config, replica.active_character_checkpoint().unwrap());
    config.install_zone_source_recovery_cache(&source).unwrap();
    let mut plain = SimulationSession::new(SimulationConfig::default());
    assert!(plain
        .install_zone_source_recovery(&source, &config)
        .is_err());
}

#[test]
fn zone_source_recovery_rebinding_retains_the_original_unknown_write_freeze() {
    let (config, mut replica) = setup();
    let mut canonical = replica.active_character_checkpoint().unwrap();
    canonical.revision += 1;
    let source = stage(&config, canonical);
    config.install_zone_source_recovery_cache(&source).unwrap();
    config
        .zone_economy_write_permit()
        .unwrap()
        .commit_outcome_unknown();
    replica
        .install_zone_source_recovery(&source, &config)
        .unwrap();
    assert!(config.ensure_account_store_writable().is_err());
    assert!(runtime_config(&replica)
        .ensure_account_store_writable()
        .is_err());
}

#[test]
fn zone_source_recovery_restores_canonical_account_permissions_and_storage_metadata() {
    let (config, mut replica) = setup();
    replica
        .app
        .world_mut()
        .resource_mut::<super::super::resources::PlayerPermissionResource>()
        .gm_level = 25;
    let mut canonical = replica.active_character_checkpoint().unwrap();
    canonical.revision += 1;
    let mut accounts = config.account_store.lock().unwrap().accounts.clone();
    let account = accounts.get_mut("demo").unwrap();
    account.gm_level = 7;
    account.storage_password = "canonical-storage-password".into();
    account.storage_size = 80;
    account.saves.insert(0, canonical);
    let source = config.zone_source_recovery_fixture(accounts, 8).unwrap();
    config.install_zone_source_recovery_cache(&source).unwrap();
    replica
        .install_zone_source_recovery(&source, &config)
        .unwrap();
    let world = replica.app.world();
    assert_eq!(
        world
            .resource::<super::super::resources::PlayerPermissionResource>()
            .gm_level,
        7
    );
    let inventory = world.resource::<super::super::resources::InventoryResource>();
    assert_eq!(inventory.storage_size, 80);
    assert!(inventory.storage_has_password);
}

#[test]
fn zone_source_recovery_rejects_a_canonical_ban_without_rebinding_the_replica() {
    let (config, mut replica) = setup();
    let mut accounts = config.account_store.lock().unwrap().accounts.clone();
    accounts.get_mut("demo").unwrap().is_banned = true;
    let source = config.zone_source_recovery_fixture(accounts, 8).unwrap();
    config.install_zone_source_recovery_cache(&source).unwrap();
    assert!(replica
        .install_zone_source_recovery(&source, &config)
        .unwrap_err()
        .contains("banned"));
    assert!(runtime_config(&replica).has_isolated_replica_bootstrap_authority());
}

fn runtime_validation_pair() -> (
    SimulationConfig,
    PreparedZoneSourceRecovery,
    CharacterSaveRecord,
) {
    let (config, mut replica) = setup();
    {
        let world = replica.app.world_mut();
        let mut skill = super::super::skills::seed_skills().remove(0);
        skill.cooldown_ends_at = 177;
        skill.cast_time_ms = 912;
        world.resource_mut::<SkillResource>().skills = vec![skill];
        world.resource_mut::<BuffResource>().buffs = vec![super::super::buffs::BuffState {
            key: "runtime-validation".into(),
            name: "test".into(),
            description: "test".into(),
            expires_at_tick: 171,
            real_time_duration: None,
            attack_bonus: 1,
            defence_bonus: 2,
            stats: vec![],
        }];
    }
    let runtime = replica.active_character_checkpoint().unwrap();
    let mut canonical = runtime.clone();
    let mut skill: Value = serde_json::from_str(&canonical.skill_states_json[0]).unwrap();
    skill["_runtimeCooldownClock"] =
        serde_json::json!({"schema": 1, "savedAtUnixMs": 0, "remainingMs": 0});
    canonical.skill_states_json[0] = skill.to_string();
    let source = stage(&config, canonical);
    (config, source, runtime)
}

#[test]
fn zone_source_recovery_runtime_validator_accepts_durable_skill_clock_and_reconstructable_private_image(
) {
    let (config, source, mut runtime) = runtime_validation_pair();
    let canonical = source.checkpoint("demo", 0).unwrap();
    assert!(canonical.skill_states_json[0].contains("_runtimeCooldownClock"));
    assert!(!runtime.skill_states_json[0].contains("_runtimeCooldownClock"));
    // Only the already-authoritative shared pools/pose and clock anchors vary.
    runtime.position.x += 3;
    runtime.direction = MirDirection::Right;
    runtime.hp = 3;
    runtime.max_hp += 10;
    runtime.mp = 4;
    runtime.max_mp += 12;
    runtime.hero_vitals = Some(crate::config::HeroVitalsState { hp: 2, mp: 3 });
    let mut skill: Value = serde_json::from_str(&runtime.skill_states_json[0]).unwrap();
    skill["cooldown_ends_at"] = serde_json::json!(900);
    skill["cast_time_ms"] = serde_json::json!(1291);
    runtime.skill_states_json[0] = skill.to_string();
    let mut buff: Value = serde_json::from_str(&runtime.buff_states_json[0]).unwrap();
    buff["expires_at_tick"] = serde_json::json!(800);
    runtime.buff_states_json[0] = buff.to_string();
    source
        .validate_runtime_checkpoint("demo", 0, &runtime)
        .unwrap();

    // This is the ordinary isolated base-snapshot seeding contract, not a PG
    // authority grant. Reconstruct from the plain runtime bytes without hooks.
    let isolated = config.fork_for_replica_apply().unwrap();
    // Production replay must select its seeded image without a PG refresh.
    assert_eq!(
        isolated.account_store_database_mode,
        AccountStoreDatabaseMode::Mirror
    );
    assert!(isolated.account_store_database_url.is_none());
    assert!(isolated.has_isolated_replica_bootstrap_authority());
    isolated
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut("demo")
        .unwrap()
        .saves
        .insert(0, runtime.clone());
    let identity = crate::ActiveSessionIdentity {
        account_id: "demo".into(),
        character_index: runtime.character.index,
        character_name: runtime.character.name.clone(),
    };
    let mut reconstructed = SimulationSession::new(isolated);
    reconstructed
        .restore_replica_session_checkpoint(&identity, &runtime)
        .unwrap();
    source
        .validate_runtime_checkpoint(
            "demo",
            0,
            &reconstructed.active_character_checkpoint().unwrap(),
        )
        .unwrap();
    assert!(reconstructed.zone_economy_write_permit().is_err());
}

#[test]
fn zone_source_recovery_runtime_validator_rejects_all_nonclock_business_changes() {
    let (_, source, runtime) = runtime_validation_pair();
    let mut cases = Vec::new();
    let mut changed = runtime.clone();
    changed.gold += 1;
    cases.push(("gold", changed));
    let mut changed = runtime.clone();
    let mut item: Value = serde_json::from_str(&changed.inventory_items_json[0]).unwrap();
    item["unique_id"] = serde_json::json!(4_000_000_001u64);
    changed.inventory_items_json[0] = item.to_string();
    cases.push(("UID", changed));
    let mut changed = runtime.clone();
    let mut skill: Value = serde_json::from_str(&changed.skill_states_json[0]).unwrap();
    skill["level"] = serde_json::json!(1);
    changed.skill_states_json[0] = skill.to_string();
    cases.push(("skill level", changed));
    let mut changed = runtime.clone();
    let mut quest = changed
        .quest_states_json
        .first()
        .map(|quest| serde_json::from_str::<Value>(quest).unwrap())
        .unwrap_or_else(|| {
            serde_json::to_value(super::super::quests::QuestState::guide_training()).unwrap()
        });
    quest["current"] = serde_json::json!(quest["current"].as_u64().unwrap() + 1);
    if changed.quest_states_json.is_empty() {
        changed.quest_states_json.push(quest.to_string());
    } else {
        changed.quest_states_json[0] = quest.to_string();
    }
    cases.push(("quest", changed));
    let mut changed = runtime.clone();
    changed.pk_points += 1;
    cases.push(("PK", changed));
    let mut changed = runtime.clone();
    let mut bind = changed
        .bind_point
        .clone()
        .unwrap_or(crate::config::CharacterBindPoint {
            map_file_name: changed.map_file_name.clone(),
            position: changed.position.clone(),
        });
    bind.position.x += 1;
    changed.bind_point = Some(bind);
    cases.push(("bind point", changed));
    let mut changed = runtime.clone();
    changed.revision += 1;
    cases.push(("revision", changed));
    let mut changed = runtime.clone();
    let mut skill: Value = serde_json::from_str(&changed.skill_states_json[0]).unwrap();
    skill["future_business_progress"] = serde_json::json!(1);
    changed.skill_states_json[0] = skill.to_string();
    cases.push(("unknown skill business field", changed));
    let mut changed = runtime.clone();
    let mut systems: Value =
        serde_json::from_str(changed.stage5_systems_json.as_deref().unwrap()).unwrap();
    systems["futureCustody"] = serde_json::json!({"quantity": 1});
    changed.stage5_systems_json = Some(systems.to_string());
    cases.push(("unknown Stage5 business field", changed));
    for (label, checkpoint) in cases {
        super::super::save::validate_character_save_record(&checkpoint).unwrap();
        assert!(
            source
                .validate_runtime_checkpoint("demo", 0, &checkpoint)
                .is_err(),
            "{label}"
        );
    }
    assert!(source
        .validate_runtime_checkpoint("foreign", 0, &runtime)
        .is_err());
    assert!(source
        .validate_runtime_checkpoint("demo", 1, &runtime)
        .is_err());
}

#[test]
fn zone_source_recovery_runtime_validator_compares_json_carriers_structurally_and_rejects_malformed_values(
) {
    let (_, source, mut runtime) = runtime_validation_pair();
    for encoded in runtime
        .inventory_items_json
        .iter_mut()
        .chain(&mut runtime.belt_items_json)
        .chain(&mut runtime.storage_items_json)
        .chain(&mut runtime.equipment_items_json)
        .chain(&mut runtime.quest_states_json)
        .chain(&mut runtime.skill_states_json)
        .chain(&mut runtime.buff_states_json)
        .chain(&mut runtime.npc_flag_states_json)
        .chain(&mut runtime.npc_saved_values_json)
        .chain(&mut runtime.npc_buy_back_items_json)
        .chain(&mut runtime.npc_used_goods_items_json)
        .chain(&mut runtime.item_rental_records_json)
        .chain(&mut runtime.hero_inventory_items_json)
        .chain(&mut runtime.hero_equipment_items_json)
    {
        *encoded =
            serde_json::to_string_pretty(&serde_json::from_str::<Value>(encoded).unwrap()).unwrap();
    }
    runtime.stage5_systems_json = runtime.stage5_systems_json.map(|encoded| {
        serde_json::to_string_pretty(&serde_json::from_str::<Value>(&encoded).unwrap()).unwrap()
    });
    source
        .validate_runtime_checkpoint("demo", 0, &runtime)
        .unwrap();
    runtime.inventory_items_json[0] = "{malformed".into();
    assert!(source
        .validate_runtime_checkpoint("demo", 0, &runtime)
        .is_err());
}

#[test]
fn zone_source_recovery_runtime_validator_allows_only_refine_timing_and_structural_nested_carriers()
{
    let (config, prior, _) = runtime_validation_pair();
    let mut canonical = prior.checkpoint("demo", 0).unwrap().clone();
    let index = canonical
        .inventory_items_json
        .iter()
        .position(|encoded| serde_json::from_str::<Value>(encoded).unwrap()["key"] == "dagger")
        .unwrap();
    let item: super::super::items::ItemState =
        serde_json::from_str(&canonical.inventory_items_json.remove(index)).unwrap();
    let mut wire = super::super::items::try_user_item_from_item_state(&item).unwrap();
    wire.refine_added = 1;
    let item = super::super::items::try_item_state_from_user_item(item, &wire).unwrap();
    let mut systems: crate::config::Stage5SystemsState =
        serde_json::from_str(canonical.stage5_systems_json.as_deref().unwrap()).unwrap();
    systems.refine = crate::config::Stage5RefineState {
        current_item: Some(item.key.clone()),
        oven_item_state_json: Some(super::super::item_custody::encode(&item).unwrap()),
        remaining_ms: 60_000,
        clock_epoch: Some("source-clock".into()),
        collect_deadline_ms: Some(90_000),
        refining: true,
        ..Default::default()
    };
    canonical.stage5_systems_json = Some(serde_json::to_string(&systems).unwrap());
    let source = stage(&config, canonical.clone());
    let mut runtime = canonical;
    let mut systems: Value =
        serde_json::from_str(runtime.stage5_systems_json.as_deref().unwrap()).unwrap();
    systems["refine"]["remainingMs"] = serde_json::json!(0);
    systems["refine"]["clockEpoch"] = serde_json::json!("target-clock");
    systems["refine"]["collectDeadlineMs"] = serde_json::json!(100);
    systems["refine"]["ready"] = serde_json::json!(true);
    let oven: Value =
        serde_json::from_str(systems["refine"]["ovenItemStateJson"].as_str().unwrap()).unwrap();
    systems["refine"]["ovenItemStateJson"] =
        serde_json::json!(serde_json::to_string_pretty(&oven).unwrap());
    runtime.stage5_systems_json = Some(systems.to_string());
    source
        .validate_runtime_checkpoint("demo", 0, &runtime)
        .unwrap();
    systems["refine"]["futureOvenBusinessValue"] = serde_json::json!(7);
    runtime.stage5_systems_json = Some(systems.to_string());
    super::super::save::validate_character_save_record(&runtime).unwrap();
    assert!(source
        .validate_runtime_checkpoint("demo", 0, &runtime)
        .is_err());
}
