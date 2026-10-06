//! Explicit offline GPU capture of source-derived mining layers and bag UI.
//! It uses real R10 map/PNG assets, the native packet/pose/atlas producers and
//! the production inventory renderer. No window, Gateway or player save.
//! The test Sprite scene consumes the resulting layer geometry directly; it
//! does not establish complete runtime/world presentation or human acceptance.

use super::*;
use bevy::{
    asset::{AssetMetaCheck, AssetPlugin},
    camera::RenderTarget,
    image::ImagePlugin,
    prelude::*,
    render::{
        pipelined_rendering::PipelinedRenderingPlugin,
        render_resource::{PollType, TextureFormat, TextureUsages},
        renderer::RenderDevice,
        view::screenshot::{Screenshot, ScreenshotCaptured},
        RenderApp, RenderPlugin,
    },
    time::TimeUpdateStrategy,
    ui::IsDefaultUiCamera,
    window::{ExitCondition, WindowPlugin},
    winit::WinitPlugin,
};
use mir2_client_bevy::{
    crystal_ui::overlays::{Mir2CrystalOverlayPlugin, NativePlayerUiState},
    inventory::{CrystalItemInfoModel, CrystalItemTooltipSourceModel, InventoryModel, ItemModel},
    native_i18n::{self, Locale},
    native_shell::{NativeShellModel, NativeShellScreen},
    read_model::UiReadModel,
};
use serde_json::json;
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};

#[derive(Resource, Default)]
struct MiningPixels(Option<Image>);
#[derive(Component)]
struct MiningActorLayer;

fn step(app: &mut App) {
    app.update();
    app.sub_app(RenderApp)
        .world()
        .resource::<RenderDevice>()
        .poll(PollType::wait_indefinitely())
        .expect("mining offscreen GPU completion");
    assert_eq!(
        app.world_mut().query::<&Window>().iter(app.world()).count(),
        0
    );
}

fn offscreen_app(root: &Path, output: &Path) -> (App, Handle<Image>) {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins
        .set(AssetPlugin { file_path: root.to_str().unwrap().into(), meta_check: AssetMetaCheck::Never, ..default() })
        .set(ImagePlugin::default_nearest())
        .set(WindowPlugin { primary_window: None, exit_condition: ExitCondition::DontExit, ..default() })
        .set(RenderPlugin { synchronous_pipeline_compilation: true, ..default() })
        .disable::<WinitPlugin>().disable::<PipelinedRenderingPlugin>()
        .disable::<bevy::audio::AudioPlugin>().disable::<bevy::app::TerminalCtrlCHandlerPlugin>())
        // The real overlay startup registers its WAV handles even in a silent
        // offscreen fixture. Keep that asset store, without opening an audio
        // device or running audio playback.
        .init_asset::<bevy::audio::AudioSource>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO))
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(mir2_client_bevy::options_effects::OptionsRuntime::with_config_path(output.join("unused-options.json")))
        .insert_resource(mir2_client_bevy::skill_binding_persistence::SkillBindingPersistenceRuntime::with_config_path(output.join("unused-bindings.json")))
        .init_resource::<MiningPixels>()
        .add_plugins(Mir2CrystalOverlayPlugin);
    crate::native_fonts::install(&mut app);
    app.insert_resource(NativeShellModel {
        screen: NativeShellScreen::InGame,
        ..default()
    });
    let mut ui = UiReadModel::default();
    ui.player.hp = 200;
    ui.player.max_hp = 200;
    ui.player.level = 12;
    app.insert_resource(ui);
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .toggle_inventory();
    let items = [
        "CopperOre",
        "IronOre",
        "SilverOre",
        "GoldOre",
        "BlackIronOre",
    ]
    .into_iter()
    .enumerate()
    .map(|(slot, name)| {
        let source = mir2_game_data::crystal_item_by_name(name).unwrap();
        let info: CrystalItemInfoModel =
            serde_json::from_value(serde_json::to_value(&source).unwrap()).unwrap();
        ItemModel {
            unique_id: Some(50_000 + slot as u64),
            key: format!("crystal-item:{}", source.item_index),
            name: source.name,
            quantity: 1,
            slot: slot as u32,
            container: 0,
            icon: source.image,
            durability_current: Some(5_000),
            durability_max: Some(10_000),
            tooltip_source: Some(CrystalItemTooltipSourceModel { info, ..default() }),
            ..default()
        }
    })
    .collect();
    app.insert_resource(InventoryModel { items, ..default() });
    let started = Instant::now();
    while app.plugins_state() != bevy::app::PluginsState::Ready {
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "GPU init timeout"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    app.finish();
    app.cleanup();
    let mut image = Image::new_target_texture(1024, 768, TextureFormat::Bgra8UnormSrgb, None);
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    app.world_mut().spawn((
        Camera2d,
        RenderTarget::Image(target.clone().into()),
        Msaa::Off,
        IsDefaultUiCamera,
    ));
    app.world_mut().spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(8.),
            top: Val::Px(8.),
            ..default()
        },
        Text::new("OFFLINE mining GPU fixture | liveAcceptance=false"),
        TextFont {
            font_size: bevy::text::FontSize::Px(12.),
            ..default()
        },
        TextColor(Color::WHITE),
    ));
    (app, target)
}

fn sprite(app: &mut App, root: &Path, path: &str, layer: &Value, rect: Option<Rect>, actor: bool) {
    let relative = path.trim_start_matches('/');
    assert!(
        root.join(relative).is_file(),
        "actual source image missing: {relative}"
    );
    let width = layer["width"].as_f64().unwrap() as f32;
    let height = layer["height"].as_f64().unwrap() as f32;
    let left = layer["left"].as_f64().unwrap() as f32;
    let top = layer["top"].as_f64().unwrap() as f32;
    let z = layer["z"].as_f64().unwrap() as f32;
    let image = app
        .world()
        .resource::<AssetServer>()
        .load::<Image>(relative.to_owned());
    let mut entity = app.world_mut().spawn((
        Sprite {
            image,
            rect,
            custom_size: Some(Vec2::new(width, height)),
            ..default()
        },
        Transform::from_xyz(
            left + width / 2. - 512.,
            384. - top - height / 2.,
            z / 100_000.,
        ),
    ));
    if actor {
        entity.insert(MiningActorLayer);
    }
}

fn map_scene(app: &mut App, root: &Path, state: &Value) -> usize {
    let mut count = 0;
    for tile in state["tiles"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|tile| tile["animationPhase"] == 0)
    {
        let page = state["atlases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|page| page["key"] == tile["atlasKey"])
            .unwrap();
        let rect = page["rects"]
            .as_array()
            .unwrap()
            .iter()
            .find(|rect| rect["key"] == tile["rectKey"])
            .unwrap();
        let number = |key| rect[key].as_f64().unwrap() as f32;
        let mut layer = tile.clone();
        // Match the production runtime's map depth band versus actor layers.
        layer["z"] = json!(tile["z"].as_f64().unwrap() * 10.);
        sprite(
            app,
            root,
            page["imageUrl"].as_str().unwrap(),
            &layer,
            Some(Rect::new(
                number("x"),
                number("y"),
                number("x") + number("width"),
                number("y") + number("height"),
            )),
            false,
        );
        count += 1;
    }
    // Additive/animated map behavior is outside this isolated pose fixture.
    for tile in state["standaloneTiles"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|tile| tile["animationPhase"] == 0 && tile["additive"] != true)
    {
        let mut layer = tile.clone();
        layer["z"] = json!(tile["z"].as_f64().unwrap() * 10.);
        sprite(
            app,
            root,
            tile["imageUrl"].as_str().unwrap(),
            &layer,
            None,
            false,
        );
        count += 1;
    }
    count
}

fn mining_pose(gender: &str, direction: &str, origin: (i32, i32), phase: u64) -> Value {
    use crate::gameplay_bridge::NativeGameplayAdapter;
    use crate::native_protocol::PacketEvent;
    let female = gender == "Female";
    let mut payload = json!({"mapFileName":"D401", "mapIndex":47, "playerObjectId":1000,
        "sceneView":{"center":{"x":origin.0,"y":origin.1},"width":19,"height":15},
        "entities":[{"objectId":1000,"kind":"selfPlayer","name":"OfflineMiner","class":"Warrior","gender":gender,
            "x":origin.0,"y":origin.1,"direction":direction,"dead":false,"ridingMount":false,
            "sprite":{"bodyLibrary":"CArmour/00","hairLibrary":"CHair/00","weaponLibrary":"CWeapon/42",
                "frameBaseOffset":if female {808} else {0},"weaponFrameOffset":if female {416} else {0},"directionStride":4}}],
        "groundDrops":[{"objectId":9001,"image":286,"x":origin.0-2,"y":origin.1+2},
            {"objectId":9002,"image":281,"x":origin.0-1,"y":origin.1+2},
            {"objectId":9003,"image":285,"x":origin.0,"y":origin.1+2},
            {"objectId":9004,"image":280,"x":origin.0+1,"y":origin.1+2},
            {"objectId":9005,"image":284,"x":origin.0+2,"y":origin.1+2}]});
    let mut adapter = NativeGameplayAdapter::default();
    adapter.observe_world_snapshot(&payload);
    let mut presentation = NativeEntityPresentation::default();
    presentation.replace_payload(payload.clone());
    presentation.sync_pending_payload(0, 0);
    presentation.mark_local_mining_request("1000", origin, direction, 1_000);
    assert!(adapter.observe_packet(&PacketEvent::Other {
        packet: "ObjectAttack".into(),
        payload: json!({"objectId":1000,"direction":direction,"spell":0,"level":0})
    }));
    adapter.apply_authoritative_overlay(&mut payload);
    presentation.replace_payload(payload);
    presentation.sync_pending_payload(1_000, 1_000);
    assert_eq!(
        presentation
            .world
            .active_state("1000")
            .unwrap()
            .pose()
            .action,
        AnimationAction::Attack2
    );
    let state = presentation
        .render_state_if_changed(1_000 + phase * 100, true)
        .unwrap();
    let owner = state["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entity| entity["objectId"] == "1000")
        .unwrap();
    assert!(owner["layers"]
        .as_array()
        .unwrap()
        .iter()
        .any(|layer| layer["key"] == "1000:weapon-primary"));
    state
}

fn assert_source_geometry(root: &Path, state: &Value) {
    let owner = state["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entity| entity["objectId"] == "1000")
        .unwrap();
    for layer in owner["layers"].as_array().unwrap() {
        let path = layer["path"].as_str().unwrap().trim_start_matches('/');
        let (library, file) = path.rsplit_once('/').unwrap();
        let index: u64 = file.trim_end_matches(".png").parse().unwrap();
        let metadata: Value =
            serde_json::from_slice(&fs::read(root.join(library).join("meta.json")).unwrap())
                .unwrap();
        let source = metadata["frames"]
            .as_array()
            .unwrap()
            .iter()
            .find(|frame| frame["index"] == index)
            .unwrap();
        assert_eq!(
            layer["left"].as_f64().unwrap(),
            480. + source["x"].as_f64().unwrap(),
            "{path} horizontal drift"
        );
        assert_eq!(
            layer["top"].as_f64().unwrap(),
            352. + source["y"].as_f64().unwrap(),
            "{path} vertical drift"
        );
        assert_eq!(layer["width"].as_f64(), source["width"].as_f64());
        assert_eq!(layer["height"].as_f64(), source["height"].as_f64());
        assert!(root.join(path).is_file(), "missing real Mine frame {path}");
    }
    assert_eq!(
        state["entities"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|entity| entity["kind"] == "item")
            .count(),
        5
    );
}

fn warm(app: &mut App) {
    let started = Instant::now();
    loop {
        step(app);
        let world = app.world_mut();
        let mut handles: Vec<_> = world
            .query::<&Sprite>()
            .iter(world)
            .map(|sprite| sprite.image.clone())
            .collect();
        handles.extend(
            world
                .query::<&ImageNode>()
                .iter(world)
                .map(|node| node.image.clone()),
        );
        let images = world.resource::<Assets<Image>>();
        let server = world.resource::<AssetServer>();
        if handles.iter().all(|handle| {
            images.contains(handle.id())
                && (handle.path().is_none() || server.is_loaded_with_dependencies(handle.id()))
        }) {
            break;
        }
        assert!(
            started.elapsed() < Duration::from_secs(40),
            "real mining/bag assets did not load"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    for _ in 0..4 {
        step(app);
    }
}

fn capture(app: &mut App, target: &Handle<Image>, path: &Path) -> Vec<u8> {
    assert!(!path.exists(), "fresh immutable output required");
    app.world_mut().resource_mut::<MiningPixels>().0 = None;
    app.world_mut()
        .spawn(Screenshot::image(target.clone()))
        .observe(
            |event: On<ScreenshotCaptured>, mut output: ResMut<MiningPixels>| {
                output.0 = Some(event.image.clone());
            },
        );
    let started = Instant::now();
    while app.world().resource::<MiningPixels>().0.is_none() {
        assert!(started.elapsed() < Duration::from_secs(15));
        step(app);
    }
    let rgba = app
        .world_mut()
        .resource_mut::<MiningPixels>()
        .0
        .take()
        .unwrap()
        .try_into_dynamic()
        .unwrap()
        .to_rgba8();
    assert_eq!(rgba.dimensions(), (1024, 768));
    rgba.save(path).unwrap();
    rgba.into_raw()
}

#[test]
#[ignore = "explicit offline mining GPU; run alone with MIR2_NATIVE_ASSET_ROOT and fresh MIR2_MINING_VISUAL_OUTPUT"]
fn mining_source_pose_and_original_ore_icons_offscreen_gpu() {
    let root = crate::assets::require_asset_root().expect("actual complete R10 root");
    let output = std::env::var_os("MIR2_MINING_VISUAL_OUTPUT")
        .map(std::path::PathBuf::from)
        .expect("explicit output");
    assert!(output.is_absolute() && !output.exists());
    fs::create_dir_all(&output).unwrap();
    let map = crate::map_parser::load_map("D401").expect("actual D401 map bytes");
    let origin = (8..i32::from(map.height) - 8)
        .find_map(|y| {
            (8..i32::from(map.width) - 8).find_map(|x| {
                (!map.cell_blocks_movement(x, y) && map.cell_blocks_movement(x + 1, y))
                    .then_some((x, y))
            })
        })
        .expect("actual walkable tile beside wall");
    let map_state = crate::map_parser::build_map_render_state_for_file(
        &map,
        crate::map_parser::MapViewport {
            center_x: origin.0,
            center_y: origin.1,
            width: 19,
            height: 15,
        },
        "D401",
    )
    .unwrap();
    let mut records = Vec::new();
    for gender in ["Male", "Female"] {
        for direction in [
            "up",
            "upright",
            "right",
            "downright",
            "down",
            "downleft",
            "left",
            "upleft",
        ] {
            for phase in 0..6 {
                let state = mining_pose(gender, direction, origin, phase);
                assert_source_geometry(&root, &state);
                records.push(
                    json!({"gender":gender,"direction":direction,"phase":phase,"state":state}),
                );
            }
        }
    }
    native_i18n::with_locale(Locale::English, || {
        let (mut app, target) = offscreen_app(&root, &output);
        let map_tiles = map_scene(&mut app, &root, &map_state);
        assert!(map_tiles > 50, "real mine terrain must render");
        let mut first_pixels = None;
        for gender in ["Male", "Female"] {
            for phase in [0, 3] {
                let previous: Vec<_> = app
                    .world_mut()
                    .query_filtered::<Entity, With<MiningActorLayer>>()
                    .iter(app.world())
                    .collect();
                for entity in previous {
                    app.world_mut().despawn(entity);
                }
                let state = mining_pose(gender, "right", origin, phase);
                for entity in state["entities"].as_array().unwrap() {
                    for layer in entity["layers"].as_array().unwrap() {
                        sprite(
                            &mut app,
                            &root,
                            layer["path"].as_str().unwrap(),
                            layer,
                            None,
                            true,
                        );
                    }
                }
                warm(&mut app);
                for index in [286, 281, 285, 280, 284] {
                    let expected = format!("original-ui/Items/{index}.png");
                    let world = app.world_mut();
                    assert!(
                        world.query::<&ImageNode>().iter(world).any(|node| node
                            .image
                            .path()
                            .is_some_and(|path| path.path().to_string_lossy() == expected)),
                        "production bag did not create ore icon {expected}"
                    );
                }
                let pixels = capture(
                    &mut app,
                    &target,
                    &output.join(format!("mine-{gender}-right-{phase}.png")),
                );
                if gender == "Male" && phase == 0 {
                    first_pixels = Some(pixels);
                } else if gender == "Male" {
                    assert_ne!(
                        pixels,
                        first_pixels.as_ref().unwrap().as_slice(),
                        "Mine frame must visibly advance"
                    );
                }
            }
        }
        fs::write(output.join("mining-visual-report.json"),serde_json::to_vec_pretty(&json!({
            "schema":"offline-mining-gpu-v1","liveAcceptance":false,"assetRoot":root,"map":"D401","origin":origin,
            "sourceGeometryCases":records.len(),"terrainSprites":map_tiles,"screenshots":4,
            "originalOreImages":5,"runtimeWorldRenderer":false,"poses":records
        })).unwrap()).unwrap();
    });
}
