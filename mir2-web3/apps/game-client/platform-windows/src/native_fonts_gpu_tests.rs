//! Explicit offscreen GPU stress tests. They create no window or connection,
//! and never read or modify player state. Run individually with --ignored.

use super::*;
use bevy::{
    camera::RenderTarget,
    prelude::*,
    render::{
        pipelined_rendering::PipelinedRenderingPlugin, render_asset::RenderAssets,
        render_resource::TextureFormat, texture::GpuImage, RenderApp, RenderPlugin,
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
    if fixed {
        install(&mut app);
    } else {
        install_named_fonts(&mut app);
    }
    while app.plugins_state() != bevy::app::PluginsState::Ready {
        std::thread::sleep(Duration::from_millis(10));
    }
    app.finish();
    app.cleanup();
    let image = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(Image::new_target_texture(
            1024,
            768,
            TextureFormat::Bgra8UnormSrgb,
            None,
        ));
    let camera = app
        .world_mut()
        .spawn((Camera2d, RenderTarget::Image(image.into())))
        .id();
    (app, camera)
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
            eprintln!("font-gpu negative-control reproduced growth baseline={baseline:?} final={current:?}");
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
