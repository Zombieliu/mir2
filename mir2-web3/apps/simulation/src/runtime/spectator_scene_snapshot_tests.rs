// Frozen full builder before the spectator projection change. This test-only
// reference preserves every field/expression independently of the projection mode.
use super::*;

fn reference_full_world_snapshot(world: &World) -> WorldSnapshot {
    let resources = world.resource::<InventoryResource>();
    let hero_inventory = world.resource::<HeroInventoryResource>();
    let player_runtime = world.resource::<PlayerRuntimeResource>();
    let map = world.resource::<MapRuntimeResource>();
    let config = &world.resource::<RuntimeConfigResource>().config;
    let session = world.resource::<SessionResource>();
    let skills = world.resource::<SkillResource>();
    let buffs = world.resource::<BuffResource>();
    let npc_state = world.resource::<NpcStateResource>();
    let stage5 = world.resource::<Stage5SystemsResource>();
    let language = session.language;
    let tooltip_viewer = session
        .selected_character
        .as_ref()
        .map(|character| (character.level, character.class));
    let hero_viewer = stage5
        .stage5_systems
        .hero
        .as_ref()
        .map(|hero| (hero.level, hero.class));
    let hero_stats = super::super::hero_stats::compute(world);
    let hero_vitals = stage5.stage5_systems.hero.as_ref().map(|_| {
        let live = hero_entity(world).and_then(|entity| entity_player_vitals(world, entity));
        let saved = hero_inventory.saved_vitals;
        crate::config::HeroVitalsSnapshot {
            hp: live
                .map(|v| v.hp)
                .or_else(|| saved.map(|v| v.hp))
                .unwrap_or(hero_stats.get(12)),
            mp: live
                .map(|v| v.mp)
                .or_else(|| saved.map(|v| v.mp))
                .unwrap_or(hero_stats.get(13)),
            max_hp: hero_stats.get(12),
            max_mp: hero_stats.get(13),
        }
    });
    let tick = super::super::session::runtime_tick(world);
    let scene_view = active_scene_view(world);
    let mut entities = collect_world_entities(
        world,
        scene_view.as_ref(),
        language,
        &resources.equipment_items,
    );
    entities.sort_by_key(|entity| entity.object_id);

    let player_object_id = current_player_object_id(world);
    let player_vitals = player_entity(world).and_then(|entity| entity_player_vitals(world, entity));
    let mut stage5_systems = stage5.stage5_systems.clone();
    if let Some(trade) = stage5_systems.trade.as_mut() {
        trade.trade_recovery_version = None;
        trade.own_items = super::super::trade_custody::project_owned_items(trade, language);
        if let Some(items) = trade.own_items.as_mut() {
            for item in items.iter_mut().flatten() {
                resolve_item_tooltip_source_for_viewer(&mut item.tooltip_source, tooltip_viewer);
            }
        }
    }
    if let Some(guild) = super::super::shared_guilds::project_legacy_shape(world) {
        stage5_systems.guild = guild;
    }
    stage5_systems.item_rental = item_rental_snapshot(world);
    super::super::refine_oven::snapshot_timer(&mut stage5_systems.refine, false);

    // Seed every known city to 0 so the HUD renders a stable row set, then
    // overlay the player's actual balances.
    let mut city_currencies: BTreeMap<String, u32> = crate::config::CITY_CURRENCY_KEYS
        .iter()
        .map(|key| ((*key).to_string(), 0))
        .collect();
    for (key, amount) in &player_runtime.city_currencies {
        city_currencies.insert(key.clone(), *amount);
    }
    let player_stats = super::super::stats::player_stats(world);
    let max_weight = u16::try_from(player_stats.bag_weight().max(0)).unwrap_or(u16::MAX);

    WorldSnapshot {
        tick,
        map_title: session
            .selected_character
            .as_ref()
            .map(|_| super::super::session::localized_map_title(language, &map.current_map.title)),
        map_file_name: session
            .selected_character
            .as_ref()
            .map(|_| map.current_map.file_name.clone()),
        in_safe_zone: player_entity(world)
            .and_then(|entity| entity_position(world, entity))
            .map(|position| is_safe_zone_point(config, map, &position))
            .unwrap_or(false),
        light_setting: current_crystal_time_of_day_lights(),
        player_object_id,
        player_hp: player_vitals.map(|vitals| vitals.hp),
        player_max_hp: player_vitals.map(|vitals| vitals.max_hp),
        player_mp: player_vitals.map(|vitals| vitals.mp),
        player_max_mp: player_vitals.map(|vitals| vitals.max_mp),
        player_crystal_stats: player_stats.snapshot_entries(),
        player_pk_points: player_runtime.pk_points,
        player_experience: player_runtime.experience,
        player_max_experience: player_runtime.max_experience,
        gold: player_runtime.gold,
        credit: player_runtime.credit,
        city_currencies,
        current_weight: current_weight(resources),
        player_weights: tooltip_viewer.and_then(|(level, class)| {
            super::super::player_weights::compute(
                resources,
                level,
                class,
                world.resource::<MountResource>().riding_mount,
            )
        }),
        max_weight,
        free_bag_slots: free_bag_slots(resources),
        max_bag_slots: crate::config::crystal_bag_slot_capacity(resources.inventory_capacity),
        inventory_capacity: resources.inventory_capacity,
        npc_gold_trade_capacity: Some(npc_gold_trade_capacity_evidence(resources)),
        storage_size: resources.storage_size,
        has_expanded_storage: resources.has_expanded_storage,
        has_storage_password: resources.storage_has_password,
        require_storage_password: storage_password_required(
            config,
            resources.storage_has_password,
            resources.storage_unlocked,
        ),
        storage_password_last_set_binary_datetime: resources
            .storage_password_last_set_binary_datetime,
        expanded_storage_expiry_time_binary_datetime: resources
            .expanded_storage_expiry_time_binary_datetime,
        scene_view: scene_view.clone(),
        terrain_patches: filter_terrain_patches(world, scene_view.as_ref()),
        decor_objects: filter_decor_objects(world, scene_view.as_ref()),
        entities,
        ground_drops: collect_ground_drops(world, scene_view.as_ref(), language),
        belt_items: resources
            .belt_items
            .iter()
            .map(|item| {
                let mut snapshot = item.snapshot(language);
                resolve_item_tooltip_source_for_viewer(
                    &mut snapshot.tooltip_source,
                    tooltip_viewer,
                );
                snapshot
            })
            .collect(),
        inventory_items: resources
            .inventory_items
            .iter()
            .map(|item| {
                let mut snapshot = item.snapshot(language);
                resolve_item_tooltip_source_for_viewer(
                    &mut snapshot.tooltip_source,
                    tooltip_viewer,
                );
                snapshot
            })
            .collect(),
        hero_inventory_items: hero_inventory
            .items
            .iter()
            .map(|item| {
                let mut snapshot = item.snapshot(language);
                resolve_item_tooltip_source_for_viewer(&mut snapshot.tooltip_source, hero_viewer);
                snapshot
            })
            .collect(),
        hero_equipment_items: hero_inventory
            .equipment
            .iter()
            .map(|item| {
                let mut snapshot = item.snapshot(language);
                resolve_item_tooltip_source_for_viewer(&mut snapshot.tooltip_source, hero_viewer);
                snapshot
            })
            .collect(),
        hero_inventory_capacity: hero_inventory.capacity,
        hero_stats: hero_stats.snapshot(),
        hero_vitals,
        hero_weights: crate::config::HeroWeightsSnapshot {
            bag: hero_stats.bag,
            wear: hero_stats.wear,
            hand: hero_stats.hand,
        },
        hero_max_experience: stage5
            .stage5_systems
            .hero
            .as_ref()
            .map(|hero| mir2_game_data::crystal_hero_settings().max_experience(hero.level)),
        native_npc_shop: super::super::npc_purchase_snapshot::current_shop(world),
        storage_items: resources
            .storage_items
            .iter()
            .map(|item| {
                let mut snapshot = item.snapshot(language);
                resolve_item_tooltip_source_for_viewer(
                    &mut snapshot.tooltip_source,
                    tooltip_viewer,
                );
                snapshot
            })
            .collect(),
        equipment_items: resources
            .equipment_items
            .iter()
            .map(|item| {
                let mut snapshot = item.snapshot(language);
                resolve_item_tooltip_source_for_viewer(
                    &mut snapshot.tooltip_source,
                    tooltip_viewer,
                );
                snapshot
            })
            .collect(),
        quest_log: quest_log_snapshots(world, language),
        active_npc_dialog: npc_state
            .active_npc_dialog
            .as_ref()
            .map(|dialog| dialog.snapshot(language)),
        npc_script_diagnostics: npc_state
            .npc_script_diagnostics
            .iter()
            .map(|diagnostic| NpcScriptDiagnosticSnapshot {
                script_key: diagnostic.script_key.clone(),
                label: diagnostic.label.clone(),
                line_number: diagnostic.line_number,
                command: diagnostic.command.clone(),
                message: diagnostic.message.clone(),
            })
            .collect(),
        known_skills: skills
            .skills
            .iter()
            .map(|skill| skill.snapshot(tick, language))
            .collect(),
        active_buffs: buffs
            .buffs
            .iter()
            .filter(|buff| !buff.expired(tick))
            .map(|buff| buff.snapshot(tick, language))
            .collect(),
        stage5_systems,
        map_transfers: collect_map_transfer_snapshots(world),
        interaction_hints: build_interaction_hints(world, resources),
    }
}

// Public fields read by Gateway's existing spectator sanitizer. Its other
// fields are identical constants for both inputs. Gateway integration tests
// additionally exercise the real sanitizer and authoritative Zone overlay.
fn spectator_payload(snapshot: &WorldSnapshot) -> serde_json::Value {
    let entities = snapshot.entities.iter().map(|entity| {
        let mut value = serde_json::to_value(entity).unwrap();
        value.as_object_mut().unwrap().insert("questIds".into(), serde_json::json!([]));
        value
    }).collect::<Vec<_>>();
    let drops = snapshot.ground_drops.iter().map(|drop| {
        let mut value = serde_json::to_value(drop).unwrap();
        if let Some(loot) = value.get_mut("loot").and_then(serde_json::Value::as_object_mut) {
            loot.remove("exactItem");
            loot.remove("exact_item");
        }
        value
    }).collect::<Vec<_>>();
    serde_json::json!({
        "tick": snapshot.tick, "mapTitle": snapshot.map_title,
        "mapFileName": snapshot.map_file_name, "inSafeZone": snapshot.in_safe_zone,
        "lightSetting": snapshot.light_setting, "sceneView": snapshot.scene_view,
        "terrainPatches": snapshot.terrain_patches, "decorObjects": snapshot.decor_objects,
        "entities": entities, "groundDrops": drops, "mapTransfers": snapshot.map_transfers,
    })
}

fn fixture() -> SimulationSession {
    let config = SimulationConfig::default();
    assert!(config.account_store_path.is_none() && config.account_store_database_url.is_none());
    let mut session = SimulationSession::new(config);
    assert!(session.try_handle_packet(ClientPacket::Login {
        account_id: "demo".into(), password: "demo".into(),
    }).unwrap().iter().any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    assert!(session.try_handle_packet(ClientPacket::StartGame { character_index: 0 }).unwrap()
        .iter().any(|packet| matches!(packet, ServerPacket::UserInformation { .. })));
    session
}

fn assert_equivalent_and_read_only(session: &SimulationSession) -> WorldSnapshot {
    assert!(session.app.world().resource::<Stage5SystemsResource>().stage5_systems.refine.oven_item_state_json.is_none(), "fixture must not contain a wall-clock refine job");
    let before_tick = session.current_tick();
    let before_save = session.active_character_checkpoint();
    let before_store = {
        let config = &session.app.world().resource::<RuntimeConfigResource>().config;
        serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap()
    };
    let before_entities = session.app.world().entities().len();
    let full = session.world_snapshot();
    assert_eq!(full, reference_full_world_snapshot(session.app.world()), "Full builder changed");
    let scene = session.spectator_world_snapshot();
    let expected = spectator_payload(&full);
    let actual = spectator_payload(&scene);
    assert_eq!(actual, expected);
    assert_eq!(serde_json::to_vec(&actual).unwrap(), serde_json::to_vec(&expected).unwrap());
    assert_eq!(session.current_tick(), before_tick);
    assert_eq!(session.app.world().entities().len(), before_entities);
    assert_eq!(session.active_character_checkpoint(), before_save, "snapshot changed durable private state");
    let after_store = {
        let config = &session.app.world().resource::<RuntimeConfigResource>().config;
        serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap()
    };
    assert!(after_store == before_store, "snapshot changed account state or save versions");
    assert_eq!(session.world_snapshot(), full, "projection changed later full owner UI");
    assert!(scene.inventory_items.is_empty() && scene.belt_items.is_empty() && scene.storage_items.is_empty());
    assert!(scene.equipment_items.is_empty() && scene.hero_inventory_items.is_empty() && scene.hero_equipment_items.is_empty());
    assert!(scene.quest_log.is_empty() && scene.known_skills.is_empty() && scene.active_buffs.is_empty());
    assert!(scene.native_npc_shop.is_none() && scene.active_npc_dialog.is_none());
    assert_eq!(scene.stage5_systems, Stage5SystemsState::default());
    scene
}

#[test]
fn spectator_scene_matches_full_before_login_selection_and_after_logout() {
    let mut session = SimulationSession::new(SimulationConfig::default());
    assert_equivalent_and_read_only(&session);
    session.try_handle_packet(ClientPacket::Login { account_id: "demo".into(), password: "demo".into() }).unwrap();
    assert_equivalent_and_read_only(&session);
    session.try_handle_packet(ClientPacket::StartGame { character_index: 0 }).unwrap();
    assert_equivalent_and_read_only(&session);
    session.try_handle_packet(ClientPacket::LogOut).unwrap();
    assert_equivalent_and_read_only(&session);
}

#[test]
fn spectator_scene_preserves_geometry_aoi_and_current_transform() {
    use mir2_game_data::{DecorKind, DecorObjectTemplate, TerrainKind, TerrainPatchTemplate};
    let mut session = fixture();
    let player = player_entity(session.app.world()).unwrap();
    let position = entity_position(session.app.world(), player).unwrap();
    {
        let mut config = session.app.world_mut().resource_mut::<RuntimeConfigResource>();
        config.config.scene_view.width = 6;
        config.config.scene_view.height = 4;
        config.config.terrain_patches = vec![
            TerrainPatchTemplate { x: position.x, y: position.y, width: 2, height: 2, kind: TerrainKind::Grass },
            TerrainPatchTemplate { x: position.x + 500, y: position.y + 500, width: 2, height: 2, kind: TerrainKind::Water },
        ];
        config.config.decor_objects = vec![
            DecorObjectTemplate { id: "inside".into(), x: position.x, y: position.y, kind: DecorKind::Lantern },
            DecorObjectTemplate { id: "outside".into(), x: position.x + 500, y: position.y + 500, kind: DecorKind::Tree },
        ];
    }
    let scene = assert_equivalent_and_read_only(&session);
    assert_eq!(scene.terrain_patches.len(), 1);
    assert_eq!(scene.decor_objects.len(), 1);
    // Trusted fixture transform, not a newly exposed normal-player command.
    let next = Point { x: position.x + 1, y: position.y };
    session.force_authoritative_player_transform(next.clone(), MirDirection::Right);
    assert_eq!(assert_equivalent_and_read_only(&session).scene_view.unwrap().center, next);
}

#[test]
fn spectator_scene_skips_private_item_ui_but_keeps_real_appearance_and_buff_inputs() {
    let mut session = fixture();
    let template = mir2_game_data::crystal_item_by_index(662).unwrap();
    let mut item = embedded_item_state_from_template(&template, ItemContainer::Bag1, 7);
    item.unique_id = 811_111;
    item.description = "private-owner-description-".repeat(256);
    {
        let mut inventory = session.app.world_mut().resource_mut::<InventoryResource>();
        inventory.inventory_items.push(item);
        inventory.equipment_items = seed_equipment_items();
        assert!(!inventory.equipment_items.is_empty());
    }
    session.app.world_mut().resource_mut::<SkillResource>().slaying_armed = true;
    session.app.world_mut().resource_mut::<Stage5SystemsResource>().stage5_systems.appearance.hair = 17;
    let actor = current_player_object_id(session.app.world()).unwrap();
    session.apply_zone_player_buff_packets(&[ServerPacket::AddBuff { buff: ClientBuff {
        buff_type: 12, object_id: actor, expire_time: 0, infinite: true,
        visible: true, paused: false, stats: Vec::new(), values: Vec::new(),
    } }], actor);
    let full = session.world_snapshot();
    assert!(!full.inventory_items.is_empty() && !full.active_buffs.is_empty());
    let scene = assert_equivalent_and_read_only(&session);
    assert_eq!(scene.entities, full.entities, "public gear/body/status projection changed");
    assert!(!serde_json::to_string(&spectator_payload(&scene)).unwrap().contains("private-owner-description-"));
}

#[test]
fn spectator_scene_preserves_exact_owned_ground_drop_until_existing_redaction() {
    let mut session = fixture();
    let player = player_entity(session.app.world()).unwrap();
    let position = entity_position(session.app.world(), player).unwrap();
    let template = mir2_game_data::crystal_item_by_index(221).unwrap();
    let state = embedded_item_state_from_template(&template, ItemContainer::Bag1, 0);
    let mut exact = user_item_from_item_state(&state); exact.unique_id = 812_222;
    let owner = current_player_object_id(session.app.world()).unwrap();
    session.app.world_mut().spawn((
        GroundDrop, ObjectId(700_003), DisplayName::literal("Fixture drop"), Position(position),
        DropOwnership { owner_object_id: owner, expires_at_tick: 9999 },
        DropPayload { quantity: 1, source_monster: "fixture".into(), source_monster_key: None,
            loot: DropLoot::InventoryItem { key: state.key, name: state.name, description: state.description, weight: state.weight,
                durability_current: state.durability_current, durability_max: state.durability_max, added_attack: 0, added_defence: 0,
                added_stats: Vec::new(), cursed: false, socket_slots: 0, show_group_pickup: false,
                exact_item: Some(crate::config::GroundDropItemPayload { item: exact, uid_assigned: true }) } },
    ));
    let scene = assert_equivalent_and_read_only(&session);
    let actual = serde_json::to_value(scene.ground_drops.iter().find(|drop| drop.object_id == 700_003).unwrap()).unwrap();
    assert!(actual["loot"].get("exactItem").is_some(), "lossless internal drop evidence lost");
    let public = spectator_payload(&scene);
    let drop = public["groundDrops"].as_array().unwrap().iter().find(|drop| drop["objectId"] == 700_003).unwrap();
    assert!(drop["loot"].get("exactItem").is_none() && drop["loot"].get("exact_item").is_none());
}

#[test]
fn spectator_scene_keeps_hero_and_summoned_pet_public_entities() {
    let mut session = fixture();
    let player = player_entity(session.app.world()).unwrap();
    let position = entity_position(session.app.world(), player).unwrap();
    let owner = current_player_object_id(session.app.world()).unwrap();
    session.app.world_mut().resource_mut::<Stage5SystemsResource>().stage5_systems.hero = Some(
        serde_json::from_value(serde_json::json!({"name":"Fixture hero","level":30,"behaviour":0,"spawned":true})).unwrap());
    session.app.world_mut().spawn((
        Hero { owner_name: "Fixture owner".into(), next_attack_tick: 0, next_move_tick: 0 }, ObjectId(700_001),
        DisplayName::literal("Fixture hero"), Position(position.clone()), Facing(MirDirection::Down),
        CharacterBody { class: MirClass::Warrior, gender: MirGender::Male, level: 30, armour_shape: Some(1), weapon_shape: Some(2) },
        PlayerVitals { hp: 120, max_hp: 150, mp: 30, max_mp: 40 },
    ));
    session.app.world_mut().spawn((
        Monster, ObjectId(700_002), DisplayName::literal("Skeleton"), Position(position.clone()), Facing(MirDirection::Down),
        MonsterAgent { image: 3, dead: false, patrol_origin: position, ai: 0, disposition: WorldEntityDisposition::Friendly,
            hostile_to_player: false, tracking_player: false, view_range: 10, can_wander: false,
            move_interval_ticks: 1, attack_interval_ticks: 1, next_move_tick: 0, next_attack_tick: 0,
            route: Vec::new(), route_index: 0, route_waiting: false, next_route_tick: 0 },
        MonsterVitals { hp: 50, max_hp: 50 },
        SummonedMonster { summoner_object_id: owner, visible_extra: true, expire_tick: None, require_summoner_within: None,
            despawn_tick_after_death: None, totem_master_object_id: None, max_minions: None },
    ));
    let scene = assert_equivalent_and_read_only(&session);
    assert!(scene.entities.iter().any(|entity| entity.object_id == 700_001));
    assert!(scene.entities.iter().any(|entity| entity.object_id == 700_002));
    assert!(scene.hero_stats.is_empty() && scene.hero_vitals.is_none());
}

#[test]
fn spectator_scene_matches_full_across_native_map_transfer() {
    let config = SimulationConfig::default().with_crystal_world_runtime().with_platinum_176_profile();
    // DogYoHyun is outside this profile. Use the same native destination as
    // active_monster_ids_match_real_map_lifecycle_and_do_not_mutate_snapshots.
    assert!(!config.map_is_allowed("DogYoHyun"));
    assert!(config.map_is_allowed("D401"));
    let mut session = SimulationSession::new(config);
    session.try_handle_packet(ClientPacket::Login { account_id: "demo".into(), password: "demo".into() }).unwrap();
    session.try_handle_packet(ClientPacket::StartGame { character_index: 0 }).unwrap();
    assert_equivalent_and_read_only(&session);
    // Existing trusted fixture API; no normal client teleport capability added.
    let packets = session.transfer_map("crystal:D401:24:181");
    assert!(packets.iter().any(|packet| matches!(packet,
        ServerPacket::MapInformation { info } if info.file_name == "D401"
    )), "native transfer did not emit destination metadata: {packets:?}");
    assert!(packets.iter().any(|packet| matches!(packet,
        ServerPacket::UserLocation { location } if location.position == (Point { x: 24, y: 181 })
    )), "native transfer did not emit the authoritative destination position: {packets:?}");
    assert_eq!(session.current_map_file_name().as_deref(), Some("D401"));
    assert_equivalent_and_read_only(&session);
}

#[test]
fn world_runtime_spectator_default_preserves_full_only_implementation() {
    use std::any::Any;
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct FullOnly { snapshot: WorldSnapshot, calls: AtomicUsize }
    impl crate::WorldRuntime for FullOnly {
        fn as_any(&self) -> &dyn Any { self }
        fn as_any_mut(&mut self) -> &mut dyn Any { self }
        fn on_connect(&self) -> Vec<ServerPacket> { Vec::new() }
        fn execute(&mut self, _: crate::WorldCommand) -> Result<Vec<ServerPacket>, String> { panic!("read-only projection executed command") }
        fn world_snapshot(&self) -> WorldSnapshot { self.calls.fetch_add(1, Ordering::SeqCst); self.snapshot.clone() }
        fn active_identity(&self) -> Option<crate::ActiveSessionIdentity> { None }
        fn save_active_character(&mut self) -> Result<(), String> { panic!("read-only projection saved state") }
        fn refresh_active_external_mail(&mut self) -> bool { panic!("read-only projection polled external state") }
    }
    let runtime = FullOnly { snapshot: fixture().world_snapshot(), calls: AtomicUsize::new(0) };
    assert_eq!(crate::WorldRuntime::spectator_world_snapshot(&runtime), runtime.snapshot);
    assert_eq!(runtime.calls.load(Ordering::SeqCst), 1);
}

