//! Explicit offscreen GPU stress tests. They create no window or connection,
//! and never read or modify player state. Run individually with --ignored.

use super::*;
use bevy::{
    camera::RenderTarget,
    prelude::*,
    render::{
        pipelined_rendering::PipelinedRenderingPlugin,
        render_asset::RenderAssets,
        render_resource::{PollType, TextureFormat, TextureUsages},
        renderer::RenderDevice,
        texture::GpuImage,
        view::screenshot::{Screenshot, ScreenshotCaptured},
        RenderApp, RenderPlugin,
    },
    text::FontAtlasSet,
    window::{ExitCondition, WindowPlugin},
    winit::WinitPlugin,
};
use std::{
    collections::BTreeSet,
    time::{Duration, Instant},
};

#[derive(Component)]
struct StressLabel;

fn app_with_offscreen_target(fixed: bool) -> (App, Entity) {
    let (app, camera, _) = app_with_font_target(fixed, false);
    (app, camera)
}

fn app_with_font_target(fixed: bool, bundled_only: bool) -> (App, Entity, Handle<Image>) {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
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
    if bundled_only {
        app.world_mut().resource_mut::<FontCx>().collection =
            fontique::Collection::new(fontique::CollectionOptions {
                system_fonts: false,
                ..default()
            });
        install_source_identity_retention(&mut app);
        install_bundled_fonts(&mut app);
        app.add_systems(
            PostUpdate,
            restore_bundled_fallbacks
                .after(bevy::text::load_font_assets_into_font_collection)
                .before(bevy::ui::UiSystems::Content),
        );
    } else if fixed {
        install(&mut app);
    } else {
        install_named_fonts(&mut app);
    }
    while app.plugins_state() != bevy::app::PluginsState::Ready {
        std::thread::sleep(Duration::from_millis(10));
    }
    app.finish();
    app.cleanup();
    let mut target = Image::new_target_texture(1024, 768, TextureFormat::Bgra8UnormSrgb, None);
    target.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let image = app.world_mut().resource_mut::<Assets<Image>>().add(target);
    let camera = app
        .world_mut()
        .spawn((Camera2d, RenderTarget::Image(image.clone().into())))
        .id();
    (app, camera, image)
}

fn rebuild_labels(app: &mut App, camera: Entity, frame: usize) {
    let previous: Vec<_> = app
        .world_mut()
        .query_filtered::<Entity, With<StressLabel>>()
        .iter(app.world())
        .collect();
    for entity in previous {
        app.world_mut().despawn(entity);
    }
    // Exercise complete disappearance long enough for Last's source expiry.
    if frame % 60 >= 56 {
        return;
    }
    for index in 0..200 {
        let (source, text, size) = match index {
            0 => (
                FontSource::Family("Arial".into()),
                "前往入口 · 自动寻路 (147,33)".to_owned(),
                12.0,
            ),
            1 => (
                FontSource::default(),
                "设为当前  查看任务详情  →  ✓".to_owned(),
                11.0,
            ),
            2 => (
                FontSource::Family("Arial".into()),
                "Online Players: 1  骷髅 ⚔ ★".to_owned(),
                10.0,
            ),
            3 => (
                FontSource::Family("Microsoft YaHei".into()),
                "当前任务 · Return to Board（334,259）".to_owned(),
                12.0,
            ),
            _ => (
                FontSource::Family("Arial".into()),
                format!("NPC {index:03}  HP {:03}/162", frame % 163),
                10.0,
            ),
        };
        app.world_mut().spawn((
            StressLabel,
            UiTargetCamera(camera),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px((index % 4) as f32 * 250.0),
                top: Val::Px((index / 4) as f32 * 15.0),
                ..default()
            },
            Text::new(text),
            TextFont {
                font: source,
                font_size: FontSize::Px(size),
                ..default()
            },
            TextColor(Color::WHITE),
            TextLayout::no_wrap(),
        ));
    }
}

fn counts(app: &App) -> (usize, usize, u64, usize, usize) {
    let fonts = app.world().resource::<FontAtlasSet>();
    let images = app.world().resource::<Assets<Image>>();
    let gpu_images = app
        .sub_app(RenderApp)
        .world()
        .resource::<RenderAssets<GpuImage>>();
    (
        fonts
            .keys()
            .map(|key| key.id)
            .collect::<BTreeSet<_>>()
            .len(),
        fonts.values().map(Vec::len).sum(),
        fonts.total_bytes(images),
        images.len(),
        gpu_images.iter().count(),
    )
}

fn run_gpu_stress(fixed: bool) {
    let (mut app, camera) = app_with_offscreen_target(fixed);
    let mut frame = 0;
    for _ in 0..12 {
        rebuild_labels(&mut app, camera, frame);
        app.update();
        frame += 1;
    }
    let baseline = counts(&app);
    assert!(
        baseline.0 >= 4 && baseline.2 > 0,
        "fallback text must actually rasterize: {baseline:?}"
    );
    assert!(
        baseline.4 >= baseline.1,
        "font textures must actually reach the GPU: {baseline:?}"
    );
    let started = Instant::now();
    let mut next_log = Duration::ZERO;
    // Match and exceed the failing game's roughly ten-minute runtime, while
    // continuously rebuilding a dense UI with genuine GPU text rendering.
    let duration = if fixed {
        Duration::from_secs(660)
    } else {
        Duration::from_secs(30)
    };
    while started.elapsed() < duration {
        let frame_start = Instant::now();
        rebuild_labels(&mut app, camera, frame);
        app.update();
        frame += 1;
        let current = counts(&app);
        if started.elapsed() >= next_log {
            eprintln!(
                "font-gpu fixed={fixed} elapsed_ms={} frames={frame} counts={current:?}",
                started.elapsed().as_millis()
            );
            next_log += Duration::from_secs(20);
        }
        if fixed {
            assert_eq!(
                (current.0, current.1, current.2),
                (baseline.0, baseline.1, baseline.2),
                "atlas growth at frame {frame}"
            );
            assert!(
                current.3 <= baseline.3 + 2 && current.4 <= baseline.4 + 2,
                "image assets must remain bounded"
            );
        } else if current.2 >= baseline.2 + 32 * 1024 * 1024 {
            eprintln!(
                "font-gpu negative-control reproduced growth baseline={baseline:?} final={current:?}"
            );
            return; // Stop far below OOM; reproducing a leak does not require crashing.
        }
        assert!(current.2 < 256 * 1024 * 1024, "abort unsafe font growth");
        if let Some(rest) = Duration::from_millis(16).checked_sub(frame_start.elapsed()) {
            std::thread::sleep(rest);
        }
    }
    assert!(fixed, "unfixed negative control did not reproduce growth");
    eprintln!(
        "font-gpu PASS duration_ms={} frames={frame} baseline={baseline:?} final={:?}",
        started.elapsed().as_millis(),
        counts(&app)
    );
}

#[test]
#[ignore = "explicit offscreen GPU negative control; run alone with --ignored"]
fn font_gpu_rebuild_negative_control() {
    run_gpu_stress(false);
}

#[test]
#[ignore = "explicit 11-minute offscreen GPU soak; run alone with --ignored"]
fn font_gpu_rebuild_soak() {
    run_gpu_stress(true);
}

#[derive(Resource, Default)]
struct CapturedFontImage(Option<Image>);

fn multilingual_gpu_step(app: &mut App) {
    app.update();
    app.sub_app(RenderApp)
        .world()
        .resource::<RenderDevice>()
        .poll(PollType::wait_indefinitely())
        .expect("offline font GPU completion");
    assert_eq!(
        app.world_mut().query::<&Window>().iter(app.world()).count(),
        0
    );
}

fn multilingual_gpu_labels(app: &mut App, camera: Entity, index: usize) {
    let previous: Vec<_> = app
        .world_mut()
        .query_filtered::<Entity, With<StressLabel>>()
        .iter(app.world())
        .collect();
    for entity in previous {
        app.world_mut().despawn(entity);
    }
    let (code, family, sample) = NINE_LANGUAGE_SAMPLES[index];
    for (row, text) in [
        format!("{code} — {family}"),
        sample.to_owned(),
        "Gold Cancel 玩家 Игрок खिलाड़ी ผู้เล่น لاعب Việt 123".to_owned(),
        "مرحبا Gold 123 عالم · سلام · क्षि · กิ้".to_owned(),
    ]
    .into_iter()
    .enumerate()
    {
        app.world_mut().spawn((
            StressLabel,
            UiTargetCamera(camera),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(24.0),
                top: Val::Px(30.0 + row as f32 * 118.0),
                width: Val::Px(976.0),
                ..default()
            },
            Text::new(text),
            TextFont {
                font: FontSource::Family(family.into()),
                font_size: FontSize::Px(24.0),
                ..default()
            },
            TextColor(Color::WHITE),
            TextLayout {
                justify: if code == "ar" {
                    bevy::text::Justify::Right
                } else {
                    bevy::text::Justify::Left
                },
                linebreak: bevy::text::LineBreak::WordOrCharacter,
            },
        ));
    }
}

#[test]
#[ignore = "explicit offline nine-language GPU capture/180-switch proof; run alone with --ignored"]
fn nine_language_bundled_fonts_gpu_shapes_and_switches_without_os_fonts() {
    let output = PathBuf::from(
        std::env::var_os("MIR2_NATIVE_NINE_FONT_GPU_OUTPUT")
            .expect("explicit fresh evidence directory"),
    );
    assert!(output.is_absolute());
    std::fs::create_dir_all(&output).unwrap();
    assert!(
        !output.join("font-gpu-evidence.json").exists(),
        "preserve prior evidence"
    );
    let (mut app, camera, target) = app_with_font_target(true, true);
    app.init_resource::<CapturedFontImage>();
    let font_assets = app.world().resource::<Assets<Font>>().len();
    let allowed: BTreeSet<_> = app
        .world()
        .resource::<Assets<Font>>()
        .iter()
        .map(|(_, font)| font.data.id())
        .collect();
    let mut evidence = Vec::new();
    for index in 0..9 {
        multilingual_gpu_labels(&mut app, camera, index);
        for _ in 0..8 {
            multilingual_gpu_step(&mut app);
        }
        let mut runs = Vec::new();
        for (text, block, info) in app
            .world_mut()
            .query::<(&Text, &ComputedTextBlock, &bevy::text::TextLayoutInfo)>()
            .iter(app.world())
        {
            assert!(!info.glyphs.is_empty());
            assert!(
                info.size.x <= 977.0,
                "font fixture overflow: {:?}",
                info.size
            );
            for line in block.buffer().lines() {
                for run in line.runs() {
                    assert!(
                        allowed.contains(&run.font().data.id()),
                        "system-only font dependency"
                    );
                    let mut glyph_count = 0;
                    for cluster in run.clusters() {
                        for glyph in cluster.glyphs() {
                            assert_ne!(glyph.id, 0, "missing glyph in {}", text.0);
                            glyph_count += 1;
                        }
                    }
                    runs.push(serde_json::json!({"source":run.font().data.id(),"rtl":run.is_rtl(),"range":[run.text_range().start,run.text_range().end],"glyphs":glyph_count}));
                }
            }
        }
        let filename = format!("font-{}.png", NINE_LANGUAGE_SAMPLES[index].0);
        let path = output.join(&filename);
        assert!(!path.exists(), "preserve prior image");
        app.world_mut()
            .spawn(Screenshot::image(target.clone()))
            .observe(
                |event: On<ScreenshotCaptured>, mut captured: ResMut<CapturedFontImage>| {
                    captured.0 = Some(event.image.clone());
                },
            );
        let start = Instant::now();
        while app.world().resource::<CapturedFontImage>().0.is_none() {
            assert!(
                start.elapsed() < Duration::from_secs(20),
                "GPU screenshot timed out"
            );
            multilingual_gpu_step(&mut app);
            std::thread::sleep(Duration::from_millis(5));
        }
        let rgba = app
            .world_mut()
            .resource_mut::<CapturedFontImage>()
            .0
            .take()
            .unwrap()
            .try_into_dynamic()
            .unwrap()
            .to_rgba8();
        assert!(
            rgba.pixels()
                .filter(|pixel| pixel[0] > 160 && pixel[1] > 160 && pixel[2] > 160)
                .count()
                > 800,
            "must render real visible text"
        );
        rgba.save(&path).unwrap();
        evidence.push(serde_json::json!({"locale":NINE_LANGUAGE_SAMPLES[index].0,"image":filename,"runs":runs}));
    }
    for _ in 0..8 {
        multilingual_gpu_step(&mut app);
    }
    let baseline = counts(&app);
    for switch in 0..180 {
        multilingual_gpu_labels(&mut app, camera, switch % 9);
        for _ in 0..3 {
            multilingual_gpu_step(&mut app);
        }
        let current = counts(&app);
        assert_eq!(
            (current.0, current.1, current.2),
            (baseline.0, baseline.1, baseline.2),
            "font atlas growth at switch {switch}"
        );
        assert_eq!(app.world().resource::<Assets<Font>>().len(), font_assets);
        assert!(current.3 <= baseline.3 + 2 && current.4 <= baseline.4 + 2);
    }
    std::fs::write(output.join("font-gpu-evidence.json"), serde_json::to_vec_pretty(&serde_json::json!({"systemFonts":false,"switches":180,"viewport":[1024,768],"fontAssets":font_assets,"baseline":baseline,"final":counts(&app),"captures":evidence})).unwrap()).unwrap();
}
