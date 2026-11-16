//! Offscreen synthetic HUD simulation fixture, not gameplay acceptance.
use bevy::{
    camera::RenderTarget,
    prelude::*,
    render::{
        pipelined_rendering::PipelinedRenderingPlugin,
        render_resource::{PollType, TextureFormat, TextureUsages},
        renderer::RenderDevice,
        view::screenshot::{Screenshot, ScreenshotCaptured},
        RenderApp, RenderPlugin,
    },
    window::{ExitCondition, WindowPlugin},
    winit::WinitPlugin,
};
use mir2_client_bevy::{
    native_i18n::NativeI18nPlugin,
    native_shell::{NativeShellModel, NativeShellScreen},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

const WIDTH: u32 = 1024;
const HEIGHT: u32 = 768;
const FIXTURE_CAPTION: &str = "SIMULATION FIXTURE: synthetic HUD; no gameplay or authentication";

#[derive(Resource, Default)]
struct CapturedImage(Option<Image>);

fn offscreen_app() -> (App, Handle<Image>, Entity) {
    let mut app = App::new();
    let public = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../web/public");
    app.add_plugins(
        DefaultPlugins
            .set(bevy::asset::AssetPlugin {
                file_path: public.to_string_lossy().into_owned(),
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
            .disable::<bevy::log::LogPlugin>()
            .disable::<bevy::audio::AudioPlugin>()
            .disable::<bevy::app::TerminalCtrlCHandlerPlugin>(),
    );
    app.insert_resource(mir2_client_bevy::native_shell_ui::NativePcUiTheme);
    app.insert_resource(mir2_client_bevy::crystal_ui::overlays::NativeUiIoPolicy::Fixture);
    app.add_plugins((
        mir2_client_bevy::crystal_ui::hud::Mir2CrystalHudPlugin,
        mir2_client_bevy::crystal_ui::chat::Mir2CrystalChatPlugin,
        mir2_client_bevy::crystal_ui::minimap::Mir2CrystalMiniMapPlugin,
        mir2_client_bevy::crystal_ui::notice::Mir2CrystalNoticePlugin,
        mir2_client_bevy::quest_ui::Mir2QuestUiPlugin,
        NativeI18nPlugin,
    ))
        .insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        })
        .insert_resource(ClearColor(Color::srgb(0.08, 0.07, 0.05)))
        .init_resource::<CapturedImage>();
    crate::pc_hud::install(&mut app).expect("compiled HUD");
    let mut model = mir2_client_bevy::read_model::UiReadModel::default();
    model.player.hp = 50;
    model.player.max_hp = 100;
    model.player.mp = 30;
    model.player.max_mp = 100;
    model.player.name = Some("SYNTHETIC FIXTURE".into());
    app.insert_resource(model);
    app.init_resource::<mir2_client_bevy::inventory::InventoryModel>();
    crate::native_fonts::install(&mut app);
    let started = Instant::now();
    while app.plugins_state() != bevy::app::PluginsState::Ready {
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "offscreen GPU initialization timed out"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    app.finish();
    app.cleanup();
    let mut texture = Image::new_target_texture(WIDTH, HEIGHT, TextureFormat::Bgra8UnormSrgb, None);
    texture.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(texture);
    let camera = app
        .world_mut()
        .spawn((
            Camera2d,
            RenderTarget::Image(target.clone().into()),
            Msaa::Off,
        ))
        .id();
    app.world_mut().spawn((
        UiTargetCamera(camera),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(16.0),
            top: Val::Px(24.0),
            width: Val::Px(992.0),
            ..default()
        },
        GlobalZIndex(20_000),
        Text::new(FIXTURE_CAPTION),
        TextFont {
            font_size: FontSize::Px(14.0),
            ..default()
        },
        TextColor(Color::WHITE),
    ));
    (app, target, camera)
}

fn step(app: &mut App) {
    app.update();
    app.sub_app(RenderApp)
        .world()
        .resource::<RenderDevice>()
        .poll(PollType::Poll)
        .expect("offscreen GPU polling");
    std::thread::sleep(Duration::from_millis(5));
}

fn target_roots(app: &mut App, camera: Entity) {
    let roots = app
        .world_mut()
        .query_filtered::<Entity, (With<Node>, Without<ChildOf>)>()
        .iter(app.world())
        .collect::<Vec<_>>();
    for entity in roots {
        app.world_mut()
            .entity_mut(entity)
            .insert(UiTargetCamera(camera));
    }
}

fn readback(app: &mut App, target: &Handle<Image>) -> Image {
    app.world_mut().resource_mut::<CapturedImage>().0 = None;
    app.world_mut()
        .spawn(Screenshot::image(target.clone()))
        .observe(
            |captured: On<ScreenshotCaptured>, mut output: ResMut<CapturedImage>| {
                output.0 = Some(captured.image.clone());
            },
        );
    let started = Instant::now();
    while app.world().resource::<CapturedImage>().0.is_none() {
        assert!(
            started.elapsed() < Duration::from_secs(15),
            "offscreen screenshot timed out"
        );
        step(app);
    }
    app.world_mut()
        .resource_mut::<CapturedImage>()
        .0
        .take()
        .unwrap()
}


pub(crate) fn capture_pc_hud_fixture() {
    use mir2_client_bevy::crystal_ui::{hud::CrystalHudAction, hud_layout::HudLayoutSpec};
    let output = PathBuf::from(std::env::var_os("MIR2_PC_HUD_VISUAL_OUT").expect("explicit output directory"));
    std::fs::create_dir_all(&output).unwrap();
    let (mut app, target, camera) = offscreen_app();
    for _ in 0..90 { step(&mut app); target_roots(&mut app, camera); }
    assert_eq!(app.world_mut().query::<&Window>().iter(app.world()).count(), 0);
    let layout = app.world().resource::<HudLayoutSpec>().resolve();
    for rect in layout.actions {
        let world = app.world_mut();
        assert!(world.query::<(&CrystalHudAction, &Node, &ComputedNode)>().iter(world)
            .any(|(_, node, computed)| node.left == Val::Px(rect.left)
                && node.top == Val::Px(rect.top) && (computed.size.x - rect.width).abs() < 1.0));
    }
    let hp = layout.hp_bar.expect("reference HP bar");
    let world = app.world_mut();
    assert!(world.query::<&Node>().iter(world).any(|node| node.left == Val::Px(hp.left + 2.0)
        && node.top == Val::Px(hp.top + 2.0) && node.width == Val::Px(((hp.width - 4.0) * 0.5).floor())));
    readback(&mut app, &target).try_into_dynamic().unwrap()
        .save(output.join("pc-hud-1024x768-simulation-fixture.png")).unwrap();
}

#[test]
#[ignore = "requires GPU and MIR2_PC_HUD_VISUAL_OUT"]
fn render_pc_hud_offscreen_simulation_fixture() { capture_pc_hud_fixture(); }
