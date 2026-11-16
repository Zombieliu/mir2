//! Explicit offscreen comparison of an identical dense animation fixture.
//! No window, network, player state or live-game input is used. This isolates
//! material batching/visibility overhead; it is not a whole-map FPS claim.
use super::*;
use bevy::{
    asset::RenderAssetUsages,
    camera::RenderTarget,
    core_pipeline::core_2d::Transparent2d,
    render::{
        gpu_readback::{Readback, ReadbackComplete},
        pipelined_rendering::PipelinedRenderingPlugin,
        render_phase::ViewSortedRenderPhases,
        render_resource::{Extent3d, PollType, TextureDimension, TextureFormat, TextureUsages},
        renderer::RenderDevice,
        RenderApp, RenderPlugin,
    },
    time::TimeUpdateStrategy,
    window::{ExitCondition, WindowPlugin},
    winit::WinitPlugin,
};
use std::time::{Duration, Instant};

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct FixtureTile {
    key: String,
    image_url: String,
    rect: Option<[f32; 4]>,
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    z: f32,
    additive: bool,
    phase: u32,
    frame_count: u32,
    tick: u32,
}

#[derive(serde::Deserialize)]
struct Fixture {
    tiles: Vec<FixtureTile>,
}

#[derive(Resource, Default)]
struct Pixels(Vec<u8>);

fn animate_before_change(
    time: Res<Time>,
    mut frames: Query<(&crate::MapAnimationFrame, &mut Visibility)>,
) {
    let count = crate::crystal_map_animation_count(time.elapsed());
    for (frame, mut visibility) in &mut frames {
        *visibility = if crate::map_animation_frame_visible(count, frame) {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

fn scene(fixed: bool) -> (App, Handle<Image>, Vec<Handle<Image>>) {
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
            .set(AssetPlugin {
                file_path: std::env::var("MIR2_MAP_GPU_ASSET_ROOT")
                    .unwrap_or_else(|_| "assets".into()),
                ..default()
            })
            .disable::<WinitPlugin>()
            .disable::<PipelinedRenderingPlugin>()
            .disable::<bevy::app::TerminalCtrlCHandlerPlugin>(),
    )
    .add_plugins(CrystalAdditiveMaterialPlugin)
    .insert_resource(ClearColor(Color::BLACK))
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO))
    .init_resource::<Pixels>();
    if fixed {
        app.add_systems(Update, crate::animate_map_tiles);
    } else {
        app.add_systems(Update, animate_before_change);
    }
    while app.plugins_state() != bevy::app::PluginsState::Ready {
        std::thread::sleep(Duration::from_millis(10));
    }
    app.finish();
    app.cleanup();
    let mut target = Image::new_target_texture(1024, 768, TextureFormat::Bgra8UnormSrgb, None);
    target.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(target);
    app.world_mut().spawn((
        Camera2d,
        RenderTarget::Image(target.clone().into()),
        Msaa::Off,
    ));
    if let Some(path) = std::env::var_os("MIR2_MAP_GPU_FIXTURE") {
        let fixture: Fixture = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let mut pending = Vec::new();
        let mesh = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(Rectangle::new(1.0, 1.0));
        for tile in fixture.tiles {
            let texture = app
                .world()
                .resource::<AssetServer>()
                .load::<Image>(tile.image_url);
            pending.push(texture.clone());
            let transform = Transform::from_xyz(
                tile.left + tile.width * 0.5 - 512.0,
                384.0 - (tile.top + tile.height * 0.5),
                tile.z * 10.0 / 100_000.0,
            );
            let entity = if tile.additive {
                let material = if fixed {
                    app.world_mut().resource_scope(
                        |world, mut cache: Mut<CrystalAdditiveMaterialCache>| {
                            cache.material(
                                &tile.key,
                                texture,
                                1.0,
                                &mut world.resource_mut::<Assets<CrystalAdditiveMaterial>>(),
                            )
                        },
                    )
                } else {
                    app.world_mut()
                        .resource_mut::<Assets<CrystalAdditiveMaterial>>()
                        .add(CrystalAdditiveMaterial {
                            tint: LinearRgba::WHITE,
                            uv_scale_offset: Vec4::new(1.0, 1.0, 0.0, 0.0),
                            texture,
                        })
                };
                app.world_mut()
                    .spawn((
                        Mesh2d(mesh.clone()),
                        MeshMaterial2d(material),
                        transform.with_scale(Vec3::new(tile.width, tile.height, 1.0)),
                    ))
                    .id()
            } else {
                app.world_mut()
                    .spawn((
                        Sprite {
                            image: texture,
                            rect: tile.rect.map(|r| Rect::new(r[0], r[1], r[2], r[3])),
                            custom_size: Some(Vec2::new(tile.width, tile.height)),
                            ..default()
                        },
                        transform,
                    ))
                    .id()
            };
            if tile.frame_count > 1 {
                app.world_mut().entity_mut(entity).insert((
                    crate::MapAnimationFrame {
                        phase: tile.phase,
                        frame_count: tile.frame_count,
                        animation_tick: tile.tick,
                    },
                    if tile.phase == 0 {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    },
                ));
            }
        }
        return (app, target, pending);
    }
    let frames: Vec<_> = (0..8)
        .map(|phase| {
            app.world_mut()
                .resource_mut::<Assets<Image>>()
                .add(Image::new_fill(
                    Extent3d {
                        width: 48,
                        height: 192,
                        depth_or_array_layers: 1,
                    },
                    TextureDimension::D2,
                    &[40 + phase * 16, 90, 160, 160],
                    TextureFormat::Rgba8UnormSrgb,
                    RenderAssetUsages::default(),
                ))
        })
        .collect();
    let mesh = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(Rectangle::new(1.0, 1.0));
    for family in 0..25 {
        for (phase, texture) in frames.iter().enumerate() {
            let material = if fixed {
                app.world_mut().resource_scope(
                    |world, mut cache: Mut<CrystalAdditiveMaterialCache>| {
                        cache.material(
                            &format!("map:{family}:phase:{phase}"),
                            texture.clone(),
                            1.0,
                            &mut world.resource_mut::<Assets<CrystalAdditiveMaterial>>(),
                        )
                    },
                )
            } else {
                app.world_mut()
                    .resource_mut::<Assets<CrystalAdditiveMaterial>>()
                    .add(CrystalAdditiveMaterial {
                        tint: LinearRgba::WHITE,
                        uv_scale_offset: Vec4::new(1.0, 1.0, 0.0, 0.0),
                        texture: texture.clone(),
                    })
            };
            app.world_mut().spawn((
                Mesh2d(mesh.clone()),
                MeshMaterial2d(material),
                Transform::from_xyz(
                    (family % 5) as f32 * 90.0 - 180.0,
                    (family / 5) as f32 * 105.0 - 210.0,
                    family as f32 * 0.001,
                )
                .with_scale(Vec3::new(48.0, 192.0, 1.0)),
                crate::MapAnimationFrame {
                    phase: phase as u32,
                    frame_count: 8,
                    animation_tick: 0,
                },
                if phase == 0 {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                },
            ));
        }
    }
    (app, target, Vec::new())
}

struct Comparison {
    materials: usize,
    draw_batches: usize,
    mean_cpu_and_gpu_completion_ms: f64,
    pixels: Vec<u8>,
}

fn compare(fixed: bool) -> Comparison {
    let (mut app, target, pending) = scene(fixed);
    let device = app
        .sub_app(RenderApp)
        .world()
        .resource::<RenderDevice>()
        .clone();
    let ready_started = Instant::now();
    while pending.iter().any(|image| {
        !app.world()
            .resource::<AssetServer>()
            .is_loaded_with_dependencies(image.id())
    }) {
        assert!(
            ready_started.elapsed() < Duration::from_secs(10),
            "local map fixture images did not load"
        );
        app.update();
        device.poll(PollType::wait_indefinitely()).unwrap();
        std::thread::sleep(Duration::from_millis(5));
    }
    for _ in 0..12 {
        app.update();
        device.poll(PollType::wait_indefinitely()).unwrap();
    }
    let start = Instant::now();
    for _ in 0..120 {
        app.update();
        device.poll(PollType::wait_indefinitely()).unwrap();
    }
    let mean_cpu_and_gpu_completion_ms = start.elapsed().as_secs_f64() * 1000.0 / 120.0;
    let materials = app
        .world()
        .resource::<Assets<CrystalAdditiveMaterial>>()
        .len();
    let draw_batches = app
        .sub_app(RenderApp)
        .world()
        .resource::<ViewSortedRenderPhases<Transparent2d>>()
        .0
        .values()
        .map(|phase| {
            phase
                .items
                .values()
                .filter(|item| !item.batch_range.is_empty())
                .count()
        })
        .sum();
    app.world_mut().spawn(Readback::texture(target)).observe(
        |event: On<ReadbackComplete>, mut pixels: ResMut<Pixels>| {
            pixels.0 = event.data.clone();
        },
    );
    for _ in 0..8 {
        app.update();
        device.poll(PollType::wait_indefinitely()).unwrap();
    }
    let pixels = app.world().resource::<Pixels>().0.clone();
    assert!(
        pixels.len() >= 1024 * 768 * 4,
        "offscreen target must really render"
    );
    assert!(pixels
        .chunks_exact(4)
        .any(|p| p[0] > 20 && p[1] > 20 && p[2] > 20));
    Comparison {
        materials,
        draw_batches,
        mean_cpu_and_gpu_completion_ms,
        pixels,
    }
}

#[test]
#[ignore = "explicit offscreen GPU comparison; run alone with --ignored --nocapture"]
fn dense_map_animation_gpu_comparison() {
    let (before, after) = if std::env::var_os("MIR2_MAP_GPU_REVERSE_ORDER").is_some() {
        let after = compare(true);
        (compare(false), after)
    } else {
        (compare(false), compare(true))
    };
    eprintln!("dense-map-fixture before materials={} batches={} completion_mean_ms={:.3}; after materials={} batches={} completion_mean_ms={:.3}; identical_pixels={}",
        before.materials, before.draw_batches, before.mean_cpu_and_gpu_completion_ms,
        after.materials, after.draw_batches, after.mean_cpu_and_gpu_completion_ms,
        before.pixels == after.pixels);
    assert!(before.materials > after.materials && after.materials > 0);
    // Transparent sorting can keep separated draws even when bindings share.
    // Material counts are evidence of reduced assets, never assumed draw/FPS gains.
    assert!(before.draw_batches >= after.draw_batches && after.draw_batches > 0);
    assert_eq!(
        before.pixels, after.pixels,
        "sharing must preserve exact blending and order"
    );
}
