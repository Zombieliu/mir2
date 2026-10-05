use super::*;

const HERO_JNI_SCENES: [&str; 6] = [
    "hero-inventory-jni",
    "hero-equipment-jni",
    "hero-status-jni",
    "hero-state-jni",
    "hero-skills-jni",
    "hero-removed-jni",
];

#[test]
fn hero_jni_scenes_require_java_inputs_instead_of_manual_shared_model_seeds() {
    for scene in HERO_JNI_SCENES {
        assert!(is_personal_jni_preview(scene), "Hero scene still seeds models: {scene}");
        assert!(SCENES.contains(&scene));
        assert_eq!(preview_panel_for_scene(scene), UiPanel::None);
    }
}

#[test]
fn hero_jni_scenes_never_replace_received_hero_or_player_authority() {
    use mir2_client_bevy::hero_model::HeroModel;
    for scene in HERO_JNI_SCENES {
        assert!(is_personal_jni_preview(scene), "Unisolated Hero scene: {scene}");
        let mut world = World::new();
        let mut sentinel = HeroModel::default();
        assert!(sentinel.apply_packet("HeroInformation", &serde_json::json!({"info":crate::hero_ingress::tests::info()})));
        let encoded = serde_json::to_value(&sentinel).unwrap();
        world.insert_resource(sentinel);
        let mut host = crate::shared_shell::HostState::default();
        host.phase = "DISCONNECTED".into();
        world.insert_resource(host);
        world.insert_resource(crate::AndroidGatewayTransportEnabled(false));
        populate_specimens(&mut world, scene);
        assert_eq!(serde_json::to_value(world.resource::<HeroModel>()).unwrap(), encoded);
        assert_eq!(world.resource::<crate::shared_shell::HostState>().phase, "DISCONNECTED");
        assert!(!world.resource::<crate::AndroidGatewayTransportEnabled>().0);
    }
}

fn received_models(scene: &str) -> (
    mir2_client_bevy::hero_model::HeroModel,
    mir2_client_bevy::inventory::InventoryModel,
    UiReadModel,
) {
    use mir2_client_bevy::inventory::CrystalUserItemModel;
    use serde_json::json;
    let snapshot = json!({"playerObjectId":42,"mapFileName":"0","mapTitle":"OFFLINE JNI Bichon",
        "entities":[{"kind":"selfPlayer","objectId":42,"name":"OFFLINE JAVA JNI","x":302,"y":634,
            "direction":"Down","class":"Warrior","gender":"Male","level":22}],
        "playerHp":80,"playerMaxHp":200,"playerMp":20,"playerMaxMp":100,"gold":777,"credit":33,
        "inventoryItems":(0..12).map(|slot|json!({"uniqueId":80000+slot,"name":format!("JNI bag {slot}"),
            "icon":100,"count":3,"slot":slot})).collect::<Vec<_>>(),
        "heroStats":[{"stat":12,"value":180},{"stat":13,"value":90}],
        "heroWeights":{"bag":7,"wear":3,"hand":2},
        "stage5Systems":{"heroLearnedMagics":[{"spell":"FireBall","key":17},{"spell":"Healing","key":18}]}});
    let mut info = crate::hero_ingress::tests::info();
    info["name"] = json!("OFFLINE JNI Hero");
    info["auto_pot"] = json!(true);
    info["inventory"][0] = serde_json::to_value(CrystalUserItemModel {
        unique_id:60000,item_index:27,count:5,current_dura:123,max_dura:456,..default()
    }).unwrap();
    info["magics"][0]["name"] = json!("JNI FireBall");
    let mut healing = info["magics"][0].clone();
    healing["name"] = json!("JNI Healing");healing["spell"] = json!("Healing");healing["key"] = json!(18);
    info["magics"].as_array_mut().unwrap().push(healing);
    let envelope = |name:&str,payload:serde_json::Value| json!({"type":"packet","packet":name,"payload":payload}).to_string();
    let mut player = crate::player_ingress::AndroidPlayerIngress::default();
    let mut hero = crate::hero_ingress::AndroidHeroIngress::default();
    assert!(hero.packet(&envelope("HeroInformation",json!({"info":info})),player.presentation_cursor()).unwrap());
    let raw = snapshot.to_string();
    let ui = serde_json::from_str(&player.snapshot(&raw).unwrap()).unwrap();
    hero.snapshot(&raw,player.presentation_cursor(),9).unwrap();
    for (name,payload) in [
        ("UpdateHeroSpawnState",json!({"state":if scene=="hero-removed-jni"{1}else{2}})),
        ("HeroHealthChanged",json!({"ownerObjectId":42,"characterName":"OFFLINE JAVA JNI","hp":67,"mp":23})),
        ("SetAutoPotValue",json!({"stat":12,"value":35})),
        ("SetAutoPotItem",json!({"grid":24,"item_index":17})),
        ("MagicLeveled",json!({"objectId":12,"spell":"FireBall","level":2,"experience":7})),
        ("SetAutoPotValue",json!({"stat":13,"value":45})),
    ] { assert!(hero.packet(&envelope(name,payload),player.presentation_cursor()).unwrap(),"{name}"); }
    let before = serde_json::to_value(hero.model()).unwrap();
    assert!(!hero.packet(&envelope("HeroHealthChanged",json!({"ownerObjectId":43,"characterName":"OFFLINE JAVA JNI",
        "hp":9999,"mp":9999})),player.presentation_cursor()).unwrap());
    assert_eq!(serde_json::to_value(hero.model()).unwrap(),before);
    let inventory = serde_json::from_value(mir2_client_bevy::native_inventory_ingress::project_native_inventory_model(&snapshot,|_,_|None)).unwrap();
    // Mac-host production ingress plus typed fixture, NOT actual Java/JNI/GPU.
    (hero.model().clone(),inventory,ui)
}

#[test]
fn hero_jni_observer_requires_every_domain_without_mixing_player_inventory_or_vitals() {
    for scene in HERO_JNI_SCENES {
        let (model,inventory,ui) = received_models(scene);
        assert!(hero_jni_models_received(scene,&model,&inventory,&ui),"{scene}");
        let mut wrong = ui.clone();wrong.player.hp=67;
        assert!(!hero_jni_models_received(scene,&model,&inventory,&wrong));
        let mut wrong = inventory.clone();wrong.items[3].unique_id=Some(u64::MAX);
        assert!(!hero_jni_models_received(scene,&model,&wrong,&ui));
        let mut wrong = model.clone();wrong.info.as_mut().unwrap().inventory.as_mut().unwrap()[3].as_mut().unwrap().count=3;
        assert!(!hero_jni_models_received(scene,&wrong,&inventory,&ui));
        let mut wrong = model.clone();wrong.spawned=!wrong.spawned;
        assert!(!hero_jni_models_received(scene,&wrong,&inventory,&ui));
        let mut wrong = model.clone();wrong.inventory_view.items.iter_mut().find(|item|item.slot==3).unwrap().tooltip_source=None;
        assert!(!hero_jni_models_received(scene,&wrong,&inventory,&ui));
        assert!(!hero_jni_models_received("hero",&model,&inventory,&ui));
    }
}

#[test]
fn hero_jni_window_entry_uses_original_observe_and_toggles_not_a_model_bootstrap() {
    for scene in HERO_JNI_SCENES {
        let (model,_,_) = received_models(scene);
        let before = serde_json::to_value(&model).unwrap();
        let mut state = NativePlayerUiState::default();
        open_hero_jni_presentation(scene,&mut state);
        assert!(!state.hero.inventory_open && !state.hero.character_open,"A missing Hero must not be fabricated");
        state.hero.observe(&model);
        assert!(!hero_jni_presentation_received(scene,&state,&model));
        open_hero_jni_presentation(scene,&mut state);
        assert!(hero_jni_presentation_received(scene,&state,&model),"{scene}");
        assert!(state.hero.pending.is_none() && state.hero.config_pending.is_none() && state.hero.assign.pending.is_none());
        assert_eq!(serde_json::to_value(&model).unwrap(),before);
        state.hero.info.as_mut().unwrap().hp=9999;
        assert!(!hero_jni_presentation_received(scene,&state,&model));
    }
}

#[test]
fn hero_jni_metadata_survives_delayed_java_events_but_resets_on_next_preview() {
    for scene in HERO_JNI_SCENES {
        let mut world = World::new();
        world.insert_resource(PreviewRequest{scene:Some(scene.into()),remaining:0});
        world.init_resource::<OfflineNpcPreviewReceipt>();
        world.init_resource::<NativePlayerUiState>();
        for _ in 0..4 { apply(&mut world); }
        assert!(world.resource::<PreviewRequest>().scene.is_none());
        assert_eq!(world.resource::<OfflineHeroJniReceipt>().scene.as_deref(),Some(scene));
        assert_eq!(world.resource::<NativeShellModel>().active_character.as_ref().unwrap().name,"OFFLINE JAVA JNI");
        *world.resource_mut::<PreviewRequest>()=PreviewRequest{scene:Some("roster".into()),remaining:0};
        apply(&mut world);
        assert!(world.resource::<OfflineHeroJniReceipt>().scene.is_none());
    }
}

#[test]
fn hero_jni_observer_waits_for_original_renderer_nodes_and_does_not_reopen_a_closed_window() {
    use bevy::ecs::system::RunSystemOnce;
    use mir2_client_bevy::crystal_ui::overlays::hero_dialog::render::HeroRoot;
    let (model,inventory,ui) = received_models("hero-inventory-jni");
    let mut world=World::new();
    world.insert_resource(OfflineHeroJniReceipt{scene:Some("hero-inventory-jni".into()),..default()});
    let mut host=crate::shared_shell::HostState::default();host.phase="IN_GAME".into();world.insert_resource(host);
    world.insert_resource(crate::AndroidGatewayTransportEnabled(true));
    let mut state=NativePlayerUiState::default();state.hero.observe(&model);world.insert_resource(state);
    world.insert_resource(model);world.insert_resource(inventory);world.insert_resource(ui);
    world.run_system_once(report_hero_jni_consumer).unwrap();
    assert!(!world.resource::<OfflineHeroJniReceipt>().opened,"An enabled network is not an offline preview");
    world.resource_mut::<crate::AndroidGatewayTransportEnabled>().0=false;
    world.run_system_once(report_hero_jni_consumer).unwrap();
    assert!(world.resource::<OfflineHeroJniReceipt>().opened);
    assert!(!world.resource::<OfflineHeroJniReceipt>().logged,"Opening alone is not renderer observation");
    world.resource_mut::<NativePlayerUiState>().hero.toggle_inventory();
    world.spawn(HeroRoot);world.spawn(HeroRoot);
    world.run_system_once(report_hero_jni_consumer).unwrap();
    assert!(!world.resource::<NativePlayerUiState>().hero.inventory_open,"Diagnostic must not override later user actions");
    assert!(!world.resource::<OfflineHeroJniReceipt>().logged);
    world.resource_mut::<NativePlayerUiState>().hero.toggle_inventory();
    world.run_system_once(report_hero_jni_consumer).unwrap();
    assert!(world.resource::<OfflineHeroJniReceipt>().logged);
    // Headless component/predicate check, never Android rendered-frame evidence.
}
