//! Explicit offline GPU fixture; no window, server, account or player save.
//!
//! Run this ignored test alone, with MIR2_PREVIEW_VISUAL_ASSET_ROOT pointing at
//! an actual package's mir2-assets and MIR2_PREVIEW_VISUAL_OUTPUT at a fresh
//! absolute output directory. The production create/select renderers and
//! preview animation system run against their real PNGs and additive material.
//! This checks rendering, not mouse/keyboard flows or live character acceptance.

use super::*;
use crate::crystal_ui::{
    overlays::{register_crystal_additive_ui, CrystalAdditiveUiMaterial},
    preview_data::{preview_frames, preview_overlay_frames, PreviewFrame},
    select::{preview_base_index, preview_render_state_for_tests, CrystalCharacterPreview},
};
use crate::native_shell::CharacterSummary;
use bevy::{
    asset::{AssetMetaCheck, AssetPlugin},
    camera::RenderTarget,
    image::ImagePlugin,
    render::{
        pipelined_rendering::PipelinedRenderingPlugin,
        render_resource::{PollType, TextureFormat, TextureUsages},
        renderer::RenderDevice,
        view::screenshot::{Screenshot, ScreenshotCaptured},
        RenderApp, RenderPlugin,
    },
    time::TimeUpdateStrategy,
    window::{ExitCondition, WindowPlugin},
    winit::WinitPlugin,
};
use serde_json::{json, Value};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const WIDTH: u32 = 1024;
const HEIGHT: u32 = 768;
const FRAME_TIME: Duration = Duration::from_millis(250);
const HALF_FRAME_TIME: Duration = Duration::from_millis(125);
const FRAME_COUNT: usize = 16;
const PREVIEW_CLASSES: [&str; 5] = ["Warrior", "Wizard", "Taoist", "Assassin", "Archer"];
const PREVIEW_GENDERS: [&str; 2] = ["Male", "Female"];

#[derive(Component)]
struct PreviewFixtureRoot;

#[derive(Resource, Default)]
struct PreviewCapturedImage(Option<Image>);

#[derive(Clone, Copy, Debug)]
enum PreviewScreen {
    Create,
    Select,
}

impl PreviewScreen {
    fn name(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Select => "select",
        }
    }

    fn anchor(self) -> (f32, f32) {
        match self {
            Self::Create => NEW_CHARACTER_PREVIEW_ANCHOR,
            Self::Select => spec::character_select::PREVIEW_ANCHOR,
        }
    }
}

fn required_absolute_path(name: &str) -> PathBuf {
    let path = std::env::var_os(name)
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("explicit offline fixture requires {name}"));
    assert!(path.is_absolute(), "{name} must be an absolute path");
    path
}

fn preflight_real_assets(root: &Path) {
    assert!(root.is_dir(), "the explicit asset root must exist");
    assert!(
        root.join("original-ui/ChrSel/meta.json").is_file(),
        "use an actual packaged ChrSel library, not synthesized fixture images"
    );
    for class in PREVIEW_CLASSES {
        for gender in PREVIEW_GENDERS {
            let base = preview_base_index(class, gender);
            let mut sets = vec![base];
            if let Some((overlay, _)) = preview_overlay_frames(base) {
                sets.push(overlay);
            }
            for set in sets {
                for frame in 0..FRAME_COUNT {
                    let path = root.join(format!("original-ui/ChrSel/{}.png", set + frame as u16));
                    assert!(
                        path.is_file(),
                        "real preview frame is missing: {}",
                        path.display()
                    );
                }
            }
        }
    }
}

fn preview_offscreen_app(root: &Path) -> (App, Handle<Image>, Entity) {
    let mut app = App::new();
    let plugins = DefaultPlugins
        .set(AssetPlugin {
            file_path: root.to_str().expect("asset root must be UTF-8").to_owned(),
            meta_check: AssetMetaCheck::Never,
            ..default()
        })
        .set(ImagePlugin::default_nearest())
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
        .disable::<bevy::app::TerminalCtrlCHandlerPlugin>();
    #[cfg(feature = "native-ui")]
    let plugins = plugins.disable::<bevy::audio::AudioPlugin>();
    app.add_plugins(plugins);
    register_crystal_additive_ui(&mut app);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO))
        .insert_resource(ClearColor(Color::srgb(0.06, 0.05, 0.03)))
        .init_resource::<PreviewCapturedImage>()
        .add_systems(Startup, load_character_preview_materials)
        .add_systems(Update, animate_character_previews);
    let initialization = Instant::now();
    while app.plugins_state() != bevy::app::PluginsState::Ready {
        assert!(
            initialization.elapsed() < Duration::from_secs(30),
            "GPU initialization timed out"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    app.finish();
    app.cleanup();
    let mut image = Image::new_target_texture(WIDTH, HEIGHT, TextureFormat::Bgra8UnormSrgb, None);
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
    // Install material resources while the animation clock remains at zero.
    preview_step(&mut app, Duration::ZERO);
    (app, target, camera)
}

fn preview_step(app: &mut App, delta: Duration) {
    app.insert_resource(TimeUpdateStrategy::ManualDuration(delta));
    app.update();
    app.sub_app(RenderApp)
        .world()
        .resource::<RenderDevice>()
        .poll(PollType::wait_indefinitely())
        .expect("offscreen GPU completion");
    assert_eq!(
        app.world().resource::<Time>().delta(),
        delta,
        "manual preview clock changed"
    );
    assert_eq!(
        app.world_mut().query::<&Window>().iter(app.world()).count(),
        0,
        "offline fixture opened a window"
    );
}

fn zero_time_drain(app: &mut App, count: usize) {
    for _ in 0..count {
        preview_step(app, Duration::ZERO);
    }
}

fn fixture_model(class: &str, gender: &str, screen: PreviewScreen) -> NativeShellModel {
    let mut model = NativeShellModel::default();
    model.screen = match screen {
        PreviewScreen::Create => NativeShellScreen::CharacterCreate,
        PreviewScreen::Select => NativeShellScreen::CharacterSelect,
    };
    model.character_create.name = "OfflinePreview".to_owned();
    model.character_create.class_name = class.to_owned();
    model.character_create.gender_name = gender.to_owned();
    model.characters = vec![CharacterSummary::new(1, "OfflinePreview", 1, class, gender)];
    model.selected_character_index = Some(1);
    model
}

fn redraw_preview_screen(
    app: &mut App,
    camera: Entity,
    model: &NativeShellModel,
    screen: PreviewScreen,
) {
    let previous: Vec<_> = app
        .world_mut()
        .query_filtered::<Entity, With<PreviewFixtureRoot>>()
        .iter(app.world())
        .collect();
    for entity in previous {
        app.world_mut().despawn(entity);
    }
    let asset_server = app.world().resource::<AssetServer>().clone();
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let mut commands = Commands::new(&mut queue, app.world());
    commands
        .spawn((
            PreviewFixtureRoot,
            UiTargetCamera(camera),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(WIDTH as f32),
                height: Val::Px(HEIGHT as f32),
                ..default()
            },
        ))
        .with_children(|parent| {
            match screen {
                PreviewScreen::Create => render_character_create(parent, &asset_server, model),
                PreviewScreen::Select => {
                    spawn_character_select_screen(parent, &asset_server, model)
                }
            }
            parent.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(8.0),
                    top: Val::Px(8.0),
                    ..default()
                },
                Text::new(format!(
                    "OFFLINE GPU fixture | liveAcceptance=false | {} / {} / {}",
                    screen.name(),
                    model.character_create.class_name,
                    model.character_create.gender_name
                )),
                TextFont {
                    font_size: bevy::text::FontSize::Px(12.0),
                    ..default()
                },
                TextColor(Color::srgb(0.85, 0.9, 0.95)),
            ));
        });
    queue.apply(app.world_mut());
}

fn screen_images_ready(app: &mut App) -> bool {
    let world = app.world_mut();
    let handles: Vec<_> = world
        .query::<&ImageNode>()
        .iter(world)
        .map(|node| node.image.clone())
        .collect();
    let assets = world.resource::<AssetServer>();
    !handles.is_empty()
        && handles
            .iter()
            .all(|handle| assets.is_loaded_with_dependencies(handle.id()))
}

fn warm_real_images(app: &mut App) {
    let started = Instant::now();
    loop {
        preview_step(app, Duration::ZERO);
        let state = preview_render_state_for_tests(app.world_mut());
        if !state.is_empty()
            && state.iter().all(|row| row["allImagesReady"] == true)
            && screen_images_ready(app)
        {
            break;
        }
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "real preview/UI assets did not load: {state:?}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    // Image/UiMaterial load completion precedes render-asset extraction.
    zero_time_drain(app, 8);
}

fn frame_metadata(base: u16, frame: usize) -> PreviewFrame {
    preview_frames(base)
        .or_else(|| match base {
            600 => preview_overlay_frames(40).map(|(_, frames)| frames),
            880 => preview_overlay_frames(320).map(|(_, frames)| frames),
            _ => None,
        })
        .expect("known Crystal preview frame set")[frame]
}

fn assert_preview_frame(
    app: &mut App,
    class: &str,
    gender: &str,
    screen: PreviewScreen,
    frame: usize,
) -> Vec<Value> {
    let state = preview_render_state_for_tests(app.world_mut());
    let body = preview_base_index(class, gender);
    let overlay = preview_overlay_frames(body).map(|(base, _)| base);
    assert_eq!(
        state.len(),
        1 + usize::from(overlay.is_some()),
        "unexpected layers: {state:?}"
    );
    assert_eq!(
        state
            .iter()
            .filter(|row| row["drivesClock"] == true)
            .count(),
        1
    );
    let anchor = screen.anchor();
    for row in &state {
        let base = row["base"].as_u64().expect("preview base") as u16;
        assert!(
            base == body || Some(base) == overlay,
            "wrong class/gender preview: {row}"
        );
        assert_eq!(row["frame"], frame, "clock or layer drift: {row}");
        assert_eq!(
            row["allImagesReady"], true,
            "a preview frame may not unload: {row}"
        );
        assert_eq!(
            row["anchor"],
            json!([anchor.0, anchor.1]),
            "control anchor changed: {row}"
        );
        let source = frame_metadata(base, frame);
        assert_eq!(
            row["rect"],
            json!([
                anchor.0 + source.x,
                anchor.1 + source.y,
                source.width,
                source.height
            ]),
            "source offsets or dimensions changed: {row}"
        );
        if base == body {
            assert_eq!(row["drivesClock"], true);
            assert_eq!(
                row["imageNode"], true,
                "body must use the actual image renderer"
            );
            assert_eq!(row["additiveNode"], false);
        } else {
            assert_eq!(row["drivesClock"], false);
            assert_eq!(
                row["imageNode"], false,
                "Wizard glow must never cover the body with ordinary alpha blending"
            );
            assert_eq!(
                row["additiveNode"], true,
                "Wizard glow must use Crystal's additive material"
            );
        }
    }
    state
}

fn capture_preview(app: &mut App, target: &Handle<Image>, path: &Path) -> Vec<u8> {
    // GPU drains and asynchronous screenshots never advance the animation.
    zero_time_drain(app, 3);
    app.world_mut().resource_mut::<PreviewCapturedImage>().0 = None;
    app.world_mut()
        .spawn(Screenshot::image(target.clone()))
        .observe(
            |captured: On<ScreenshotCaptured>, mut output: ResMut<PreviewCapturedImage>| {
                output.0 = Some(captured.image.clone());
            },
        );
    let started = Instant::now();
    while app.world().resource::<PreviewCapturedImage>().0.is_none() {
        assert!(
            started.elapsed() < Duration::from_secs(15),
            "offscreen screenshot timed out"
        );
        preview_step(app, Duration::ZERO);
        std::thread::sleep(Duration::from_millis(5));
    }
    let rgba = app
        .world_mut()
        .resource_mut::<PreviewCapturedImage>()
        .0
        .take()
        .unwrap()
        .try_into_dynamic()
        .expect("GPU image to PNG")
        .to_rgba8();
    assert_eq!(rgba.dimensions(), (WIDTH, HEIGHT));
    rgba.save(path).expect("write actual GPU screenshot");
    rgba.into_raw()
}

fn compare_wizard_additive(
    app: &mut App,
    target: &Handle<Image>,
    output: &Path,
    filename: &str,
    glow: &[u8],
    state: &[Value],
) -> Value {
    let overlays: Vec<_> = app
        .world_mut()
        .query_filtered::<Entity, (
            With<CrystalCharacterPreview>,
            With<MaterialNode<CrystalAdditiveUiMaterial>>,
        )>()
        .iter(app.world())
        .collect();
    assert_eq!(overlays.len(), 1);
    // Test-only presentation masking; the production component/material and
    // retained clock are not replaced, and the exact same frame is compared.
    app.world_mut()
        .entity_mut(overlays[0])
        .insert(Visibility::Hidden);
    let body_only = capture_preview(
        app,
        target,
        &output.join(format!("{filename}-body-only.png")),
    );
    app.world_mut()
        .entity_mut(overlays[0])
        .insert(Visibility::Inherited);
    zero_time_drain(app, 3);
    let restored = preview_render_state_for_tests(app.world_mut());
    assert_eq!(restored.as_slice(), state);
    let overlay = state
        .iter()
        .find(|row| row["additiveNode"] == true)
        .unwrap();
    let rect: Vec<f64> = overlay["rect"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_f64().unwrap())
        .collect();
    let left = rect[0].floor().max(0.0) as u32;
    let top = rect[1].floor().max(0.0) as u32;
    let right = (rect[0] + rect[2]).ceil().min(f64::from(WIDTH)) as u32;
    let bottom = (rect[1] + rect[3]).ceil().min(f64::from(HEIGHT)) as u32;
    let mut brightened = 0;
    let mut darkened = 0;
    let mut max_channel_drop = 0;
    for y in top..bottom {
        for x in left..right {
            let pixel = ((y * WIDTH + x) * 4) as usize;
            let mut brighter = false;
            let mut darker = false;
            for channel in 0..3 {
                let before = body_only[pixel + channel];
                let after = glow[pixel + channel];
                brighter |= after > before.saturating_add(2);
                darker |= before > after.saturating_add(2);
                max_channel_drop = max_channel_drop.max(before.saturating_sub(after));
            }
            brightened += usize::from(brighter);
            darkened += usize::from(darker);
        }
    }
    assert_eq!(
        darkened, 0,
        "additive glow darkened {darkened} pixels in {filename}; max drop {max_channel_drop}"
    );
    assert!(
        brightened > 10,
        "Wizard overlay was not visibly rendered in {filename} ({brightened} pixels)"
    );
    json!({"bodyOnlyFile":format!("{filename}-body-only.png"), "comparedRect":[left,top,right,bottom], "brightenedPixels":brightened, "darkenedPixels":darkened, "maxChannelDrop":max_channel_drop, "quantizationTolerance":2})
}

fn write_fixture_report(path: &Path, cases: &[Value], complete: bool) {
    let report = json!({
        "kind":"offline_production_character_preview_gpu_fixture",
        "liveAcceptance":false,
        "complete":complete,
        "passed":complete,
        "serverConnected":false,
        "windowCreated":false,
        "frameDurationMs":250,
        "framesPerSet":16,
        "classGenderSets":10,
        "screens":["create","select"],
        "interactiveCreationClasses":["Warrior","Wizard","Taoist"],
        "limits":["Assassin and Archer use explicit offline model presentation; this does not make them available to create online", "No keyboard/mouse input, login, reconnect, character creation or player save was exercised"],
        "cases":cases,
    });
    fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
}

#[test]
#[ignore = "explicit real-asset offscreen GPU screenshots; run alone with MIR2_PREVIEW_VISUAL_ASSET_ROOT and MIR2_PREVIEW_VISUAL_OUTPUT"]
fn all_character_previews_animate_and_wizard_glow_adds_offscreen() {
    let assets = required_absolute_path("MIR2_PREVIEW_VISUAL_ASSET_ROOT")
        .canonicalize()
        .expect("real asset root");
    preflight_real_assets(&assets);
    let output = required_absolute_path("MIR2_PREVIEW_VISUAL_OUTPUT");
    assert!(
        !output.starts_with(&assets),
        "evidence must not modify the asset package"
    );
    fs::create_dir_all(&output).unwrap();
    let output = output.canonicalize().unwrap();
    assert!(
        !output.starts_with(&assets),
        "evidence must not modify the asset package"
    );
    let report_path = output.join("preview-report.json");
    let _new_report = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&report_path)
        .expect("use a fresh output directory; never replace prior fixture evidence");
    let mut frames: File = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("preview-frames.jsonl"))
        .unwrap();
    let mut cases = Vec::new();
    write_fixture_report(&report_path, &cases, false);
    let (mut app, target, camera) = preview_offscreen_app(&assets);
    for class in PREVIEW_CLASSES {
        for gender in PREVIEW_GENDERS {
            for screen in [PreviewScreen::Create, PreviewScreen::Select] {
                let mut model = fixture_model(class, gender, screen);
                redraw_preview_screen(&mut app, camera, &model, screen);
                warm_real_images(&mut app);
                let mut captures = Vec::new();
                let mut redraw_verified = false;
                for frame in 0..FRAME_COUNT {
                    let state = assert_preview_frame(&mut app, class, gender, screen, frame);
                    writeln!(frames, "{}", json!({"class":class,"gender":gender,"screen":screen.name(),"frame":frame,"offline":true,"liveAcceptance":false,"layers":state})).unwrap();
                    frames.flush().unwrap();
                    if frame == 0 || frame == 8 || (class == "Wizard" && [4, 12].contains(&frame)) {
                        let filename = format!(
                            "{}-{}-{}-frame-{frame:02}",
                            screen.name(),
                            class.to_ascii_lowercase(),
                            gender.to_ascii_lowercase()
                        );
                        let pixels = capture_preview(
                            &mut app,
                            &target,
                            &output.join(format!("{filename}.png")),
                        );
                        assert_eq!(
                            assert_preview_frame(&mut app, class, gender, screen, frame),
                            state,
                            "screenshot advanced the clock"
                        );
                        let comparison = (class == "Wizard").then(|| {
                            compare_wizard_additive(
                                &mut app, &target, &output, &filename, &pixels, &state,
                            )
                        });
                        captures.push(json!({"file":format!("{filename}.png"),"frame":frame,"additiveComparison":comparison}));
                    }
                    if frame == 5 {
                        preview_step(&mut app, HALF_FRAME_TIME);
                        assert_preview_frame(&mut app, class, gender, screen, frame);
                        model.character_create.name.push('2');
                        model.character_create.focus = CharacterCreateFocus::Gender;
                        model.characters[0].name.push('2');
                        redraw_preview_screen(&mut app, camera, &model, screen);
                        warm_real_images(&mut app);
                        assert_preview_frame(&mut app, class, gender, screen, frame);
                        // The other 125ms must finish this existing frame, not
                        // start a new timer after a name/focus redraw.
                        preview_step(&mut app, HALF_FRAME_TIME);
                        assert_preview_frame(&mut app, class, gender, screen, frame + 1);
                        redraw_verified = true;
                    } else {
                        preview_step(&mut app, FRAME_TIME);
                    }
                }
                assert_preview_frame(&mut app, class, gender, screen, 0);
                assert!(redraw_verified);
                cases.push(json!({"class":class,"gender":gender,"screen":screen.name(),"validatedFrames":16,"loopReturnedToFrame":0,"nameFocusRedrawPreservedFrame":true,"redrawPreservedHalfFrameClock":true,"captures":captures}));
                write_fixture_report(&report_path, &cases, false);
            }
        }
    }
    assert_eq!(cases.len(), 20);
    write_fixture_report(&report_path, &cases, true);
    eprintln!(
        "offline character preview GPU evidence saved: {} (liveAcceptance=false)",
        output.display()
    );
}
