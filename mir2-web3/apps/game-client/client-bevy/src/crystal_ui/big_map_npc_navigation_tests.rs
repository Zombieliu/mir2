use crate::big_map::{BigMapInfo, BigMapNpc};
use crate::quest_ui::{QuestRouteNavigationIntentQueue, QuestRouteTarget};
use mir2_ui_core::state::UiPanel;

fn npc_model() -> BigMapModel {
    let mut model = BigMapModel::default();
    model.set_current_map(1);
    model.apply_new_map_info(
        1,
        BigMapInfo {
            title: "BichonProvince".into(),
            width: 800,
            height: 800,
            big_map: 101,
            movements: vec![],
            npcs: (1..=2)
                .map(|id| BigMapNpc {
                    index: id as i32,
                    file_name: "Blacksmith".into(),
                    name: format!("Blacksmith {id}"),
                    map_index: 1,
                    location: BigMapPoint { x: 296, y: 613 },
                    image: 0,
                    rate: 0,
                    show_on_big_map: true,
                    big_map_icon: 0,
                    object_id: id,
                    icon: 0,
                    can_teleport_to: false,
                })
                .collect(),
        },
    );
    model
}

fn button_entity(app: &mut App, action: OverlayButton) -> Entity {
    let world = app.world_mut();
    world
        .query::<(Entity, &OverlayButton)>()
        .iter(world)
        .find(|(_, candidate)| **candidate == action)
        .unwrap()
        .0
}

#[test]
fn big_map_npc_row_click_highlights_selection_and_go_to_queues_ordinary_navigation() {
    let mut app = overlay_render_test_app();
    init_overlay_button_test_resources(&mut app);
    app.init_resource::<NativePlayerUiIntentQueue>()
        .init_resource::<PendingOperations>()
        .init_resource::<NativeUiIntentQueue>()
        .init_resource::<QuestRouteNavigationIntentQueue>()
        .add_systems(Update, process_overlay_buttons.before(render_overlays));
    app.insert_resource(npc_model());
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .core
        .panel = UiPanel::BigMap;
    app.update();
    let row = button_entity(&mut app, OverlayButton::SelectBigMapNpc(2));
    *app.world_mut().get_mut::<Interaction>(row).unwrap() = Interaction::Pressed;
    app.update();
    assert_eq!(
        app.world().resource::<BigMapModel>().selected_npc_object_id,
        Some(2)
    );
    let selected = button_entity(&mut app, OverlayButton::SelectBigMapNpc(2));
    let other = button_entity(&mut app, OverlayButton::SelectBigMapNpc(1));
    let world = app.world();
    assert_ne!(
        world.get::<BackgroundColor>(selected),
        world.get::<BackgroundColor>(other)
    );
    let selected_label = world
        .get::<Children>(selected)
        .unwrap()
        .iter()
        .next()
        .unwrap();
    let other_label = world.get::<Children>(other).unwrap().iter().next().unwrap();
    assert_ne!(
        world.get::<TextColor>(selected_label),
        world.get::<TextColor>(other_label)
    );
    let first = world.get::<Node>(other).unwrap();
    let second = world.get::<Node>(selected).unwrap();
    assert_eq!(
        (first.top, first.height, second.top),
        (Val::Px(50.), Val::Px(21.), Val::Px(71.))
    );
    let go_to = button_entity(&mut app, OverlayButton::BigMapNavigate);
    assert!(
        app.world()
            .get::<CrystalImageButton>(go_to)
            .unwrap()
            .enabled
    );
    assert!(app.world().get::<Button>(go_to).is_some());
    *app.world_mut().get_mut::<Interaction>(go_to).unwrap() = Interaction::Pressed;
    app.update();
    assert!(!app.world().resource::<NativePlayerUiState>().bigmap_open());
    let intent = app
        .world_mut()
        .resource_mut::<QuestRouteNavigationIntentQueue>()
        .take()
        .unwrap();
    assert_eq!(intent.target, QuestRouteTarget::MapNpc { object_id: 2 });
    assert_eq!(
        (intent.quest_index, intent.map_index, intent.x, intent.y),
        (0, 1, 296, 613)
    );
    assert!(intent.matches_big_map_npc_destination(app.world().resource::<BigMapModel>()));
    assert!(app
        .world_mut()
        .resource_mut::<BigMapGatewayIntentQueue>()
        .drain_intents()
        .into_iter()
        .all(|intent| !matches!(
            intent,
            crate::big_map::BigMapGatewayIntent::TeleportToNpc { .. }
        )));
}

#[test]
fn big_map_go_to_rejects_remote_or_removed_selection_without_closing_the_map() {
    for invalid in ["removed", "remote", "world", "zero"] {
        let mut app = help_button_test_app();
        app.init_resource::<QuestRouteNavigationIntentQueue>();
        let mut model = npc_model();
        assert!(model.select_npc(2));
        match invalid {
            "removed" => model.maps.get_mut(&1).unwrap().info.npcs.clear(),
            "remote" => model.active_map_index = Some(2),
            "world" => model.view = crate::big_map::BigMapView::WorldMap,
            "zero" => model.selected_npc_object_id = Some(0),
            _ => unreachable!(),
        }
        app.insert_resource(model);
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .core
            .panel = UiPanel::BigMap;
        press_help_button(&mut app, OverlayButton::BigMapNavigate);
        assert!(
            app.world()
                .resource::<QuestRouteNavigationIntentQueue>()
                .is_empty(),
            "{invalid}"
        );
        assert!(
            app.world().resource::<NativePlayerUiState>().bigmap_open(),
            "{invalid}"
        );
    }
}

/// Explicit production-renderer screenshot, with no window or Gateway session.
#[test]
#[ignore = "offscreen GPU screenshot; run alone with an original asset root"]
fn big_map_npc_selected_row_offscreen_visual() {
    use bevy::camera::RenderTarget;
    use bevy::render::{
        pipelined_rendering::PipelinedRenderingPlugin,
        render_resource::{PollType, TextureFormat, TextureUsages},
        renderer::RenderDevice,
        view::screenshot::{Screenshot, ScreenshotCaptured},
        RenderApp, RenderPlugin,
    };
    use bevy::window::{ExitCondition, WindowPlugin};
    use bevy::winit::WinitPlugin;
    use std::time::{Duration, Instant};

    #[derive(Resource, Default)]
    struct Capture(Option<Image>);
    fn step(app: &mut App) {
        app.update();
        app.sub_app(RenderApp)
            .world()
            .resource::<RenderDevice>()
            .wgpu_device()
            .poll(PollType::wait_indefinitely())
            .unwrap();
    }
    let assets =
        std::env::var("MIR2_NPC_VISUAL_ASSET_ROOT").expect("original assets must be explicit");
    let previous_locale = crate::native_i18n::locale();
    crate::native_i18n::activate(crate::native_i18n::Locale::TraditionalChinese);
    let output =
        std::path::PathBuf::from(std::env::var_os("MIR2_NPC_VISUAL_OUTPUT").expect("owned output"));
    std::fs::create_dir_all(&output).unwrap();
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                file_path: assets,
                ..default()
            })
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            })
            .set(RenderPlugin {
                synchronous_pipeline_compilation: true,
                ..default()
            })
            .disable::<WinitPlugin>()
            .disable::<PipelinedRenderingPlugin>()
            .disable::<bevy::audio::AudioPlugin>()
            .disable::<bevy::app::TerminalCtrlCHandlerPlugin>(),
    )
    .insert_resource(ClearColor(Color::srgb(0.06, 0.05, 0.03)))
    .init_resource::<Capture>()
    .add_systems(
        Update,
        super::super::super::item_image::layout_original_item_images,
    );
    let started = Instant::now();
    while app.plugins_state() != bevy::app::PluginsState::Ready {
        assert!(started.elapsed() < Duration::from_secs(30));
        std::thread::sleep(Duration::from_millis(10));
    }
    app.finish();
    app.cleanup();
    let mut image = Image::new_target_texture(1024, 768, TextureFormat::Bgra8UnormSrgb, None);
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    let camera = app
        .world_mut()
        .spawn((
            Camera2d,
            RenderTarget::Image(target.clone().into()),
            Msaa::Off,
        ))
        .id();
    let mut model = npc_model();
    let info = &mut model.maps.get_mut(&1).unwrap().info;
    info.npcs[0].name = "Blacksmith_Bill".into();
    info.npcs[0].location = BigMapPoint { x: 302, y: 221 };
    info.npcs[1].name = "Blacksmith_Smith".into();
    assert!(model.select_npc(2));
    let renderer = BigMapUiState::default();
    let ui = UiReadModel::default();
    let server = app.world().resource::<AssetServer>().clone();
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let mut commands = Commands::new(&mut queue, app.world());
    let map_root = commands
        .spawn((
            Node {
                width: Val::Px(1024.),
                height: Val::Px(768.),
                ..default()
            },
            UiTargetCamera(camera),
        ))
        .with_children(|root| {
            render_bigmap(root, Some(&server), &model, &renderer, &ui);
        })
        .id();
    queue.apply(app.world_mut());
    for _ in 0..36 {
        step(&mut app);
    }
    app.world_mut()
        .spawn(Screenshot::image(target.clone()))
        .observe(
            |captured: On<ScreenshotCaptured>, mut capture: ResMut<Capture>| {
                capture.0 = Some(captured.image.clone());
            },
        );
    let started = Instant::now();
    while app.world().resource::<Capture>().0.is_none() {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "GPU capture timed out"
        );
        step(&mut app);
    }
    let screenshot = app.world_mut().resource_mut::<Capture>().0.take().unwrap();
    screenshot
        .try_into_dynamic()
        .unwrap()
        .save(output.join("selected-smith.png"))
        .unwrap();
    app.world_mut().despawn(map_root);
    let shop = ShopModel {
        service_mode: NpcShopServiceMode::Buy,
        goods: (1..=19)
            .map(|id| ShopGood {
                unique_id: id,
                name: if id == 19 { "PickAxe" } else { "WoodenSword" }.into(),
                price: if id == 19 { 2500 } else { 50 },
                icon: if id == 19 { 50 } else { 1 },
                count: 1,
                stock: 99,
                ..default()
            })
            .collect(),
        ..default()
    };
    let shop_ui = ShopUiState {
        start_index: 11,
        ..default()
    };
    let inventory = InventoryModel {
        gold: 874,
        ..default()
    };
    let state = NativePlayerUiState {
        npc_shop_buy_tab: true,
        ..default()
    };
    let player = crate::read_model::PlayerStats {
        level: 12,
        ..default()
    };
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let mut commands = Commands::new(&mut queue, app.world());
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(NPC_GOODS_PANEL_ORIGIN.x as f32),
                top: Val::Px(NPC_GOODS_PANEL_ORIGIN.y as f32),
                width: Val::Px(NPC_GOODS_PANEL_SIZE.width as f32),
                height: Val::Px(NPC_GOODS_PANEL_SIZE.height as f32),
                ..default()
            },
            UiTargetCamera(camera),
        ))
        .with_children(|root| {
            render_shop(
                root,
                Some(&server),
                &shop,
                &shop_ui,
                &inventory,
                &state,
                &player,
            );
        });
    queue.apply(app.world_mut());
    for _ in 0..36 {
        step(&mut app);
    }
    let world = app.world_mut();
    assert_eq!(
        world
            .query_filtered::<&Node, With<super::super::super::item_image::OriginalItemImage>>()
            .iter(world)
            .filter(|node| node.display == Display::Flex)
            .count(),
        8,
        "all eight last-page original item icons must actually be visible"
    );
    assert!(
        world
            .query::<&Text>()
            .iter(world)
            .any(|text| text.0 == "鶴嘴鋤"),
        "the packaged Traditional Chinese pickaxe label must be present"
    );
    app.world_mut().spawn(Screenshot::image(target)).observe(
        |captured: On<ScreenshotCaptured>, mut capture: ResMut<Capture>| {
            capture.0 = Some(captured.image.clone());
        },
    );
    let started = Instant::now();
    while app.world().resource::<Capture>().0.is_none() {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "shop GPU capture timed out"
        );
        step(&mut app);
    }
    let screenshot = app.world_mut().resource_mut::<Capture>().0.take().unwrap();
    screenshot
        .try_into_dynamic()
        .unwrap()
        .save(output.join("shop-last-page.png"))
        .unwrap();
    std::fs::write(output.join("scope.json"), serde_json::to_vec_pretty(&serde_json::json!({
        "kind":"offscreen_production_renderer_fixture", "windowInput":false, "liveAcceptance":false,
        "selectedNpc":"Blacksmith_Smith", "paidTeleportEligible":false, "goToEnabled":true,
        "width":1024, "height":768,
        "shopGoods":"synthetic 19-item renderer fixture", "shopStartIndex":11,
        "shopLastItem":"PickAxe", "shopGold":874, "shopPrice":2500
    })).unwrap()).unwrap();
    crate::native_i18n::activate(previous_locale);
}
