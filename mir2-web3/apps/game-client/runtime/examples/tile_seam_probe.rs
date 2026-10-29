//! GPU readback probe for fractional-camera seams between independently rendered map tiles.
//!
//! This headless example renders opaque 48x32 atlas tiles with transparent gutters into
//! image targets. It compares `Msaa::Sample4` and `Msaa::Off` at the fractional camera
//! positions that occur during smooth motion, then reports actual screenshot readback data.

use std::collections::BTreeMap;

use bevy::{
    app::SubApps,
    asset::RenderAssetUsages,
    camera::{visibility::RenderLayers, ClearColorConfig, RenderTarget},
    image::{Image, ImagePlugin, TextureAtlas, TextureAtlasLayout},
    math::{URect, UVec2},
    prelude::*,
    render::{
        render_resource::{Extent3d, PollType, TextureDimension, TextureFormat, TextureUsages},
        renderer::RenderDevice,
        view::screenshot::{Screenshot, ScreenshotCaptured},
        RenderPlugin,
    },
    window::ExitCondition,
    winit::WinitPlugin,
};

const TARGET_WIDTH: u32 = 96;
const TARGET_HEIGHT: u32 = 64;
const TILE_WIDTH: u32 = 48;
const TILE_HEIGHT: u32 = 32;
const ATLAS_SIZE: u32 = 128;
const WARMUP_UPDATES: usize = 8;
const MAX_PUMP_FRAMES: usize = 240;
const RESULTS_PATH: &str = "C:/mir2-ui-repair-20260921/tile-seam-probe-results.json";

const ATLAS_RECTS: [(u32, u32, u32, u32); 4] = [
    (8, 8, 56, 40),
    (72, 8, 120, 40),
    (8, 72, 56, 104),
    (72, 72, 120, 104),
];
const TILE_COLORS: [[u8; 4]; 4] = [
    [235, 48, 48, 255],
    [48, 210, 78, 255],
    [48, 104, 235, 255],
    [230, 205, 48, 255],
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Gutter {
    Transparent,
    EdgeExtruded,
}

impl Gutter {
    const fn name(self) -> &'static str {
        match self {
            Self::Transparent => "transparent",
            Self::EdgeExtruded => "edgeExtruded",
        }
    }
}

#[derive(Clone)]
struct ProbeCase {
    label: String,
    offset: Vec2,
    msaa: Msaa,
    gutter: Gutter,
}

#[derive(Resource, Clone)]
struct ProbeCases(Vec<ProbeCase>);

#[derive(Resource, Default, Clone)]
struct ProbeResults {
    frames: BTreeMap<usize, Vec<u8>>,
}

#[derive(Resource, Default)]
struct ProbeTargets(Vec<Handle<Image>>);

#[derive(Resource, Default)]
struct CaptureState {
    warmup_updates: usize,
    scheduled: bool,
}

fn main() {
    let cases = probe_cases();
    let mut app = HeadlessProbe::new(cases.clone());

    for _ in 0..MAX_PUMP_FRAMES {
        app.update();
        if app.result_count() == cases.len() {
            break;
        }
    }

    let results = app.results();
    if results.frames.len() != cases.len() {
        finish(render_timeout_json(cases.len(), results.frames.len()), 2);
    }

    let analyzed: Vec<_> = cases
        .iter()
        .enumerate()
        .map(|(index, case)| FrameObservation::analyze(case, &results.frames[&index]))
        .collect();
    if analyzed
        .iter()
        .any(|observation| !observation.has_opaque_interior())
    {
        finish(
            render_observations_json(
                &analyzed,
                "inconclusive",
                "missing_opaque_color_or_alpha_positive_control",
                &[],
            ),
            2,
        );
    }
    let zero_seam_configurations = zero_seam_configurations(&analyzed);
    let (outcome, exit_code) = if zero_seam_configurations.is_empty() {
        ("no_zero_seam_configuration", 1)
    } else {
        ("zero_seam_configuration_found", 0)
    };
    finish(
        render_observations_json(&analyzed, "completed", outcome, &zero_seam_configurations),
        exit_code,
    );
}

fn finish(report: String, exit_code: i32) -> ! {
    if let Err(error) = std::fs::write(RESULTS_PATH, &report) {
        eprintln!("tile seam probe could not write {RESULTS_PATH}: {error}");
        std::process::exit(2);
    }
    println!("{report}");
    std::process::exit(exit_code);
}

struct HeadlessProbe(SubApps);

impl HeadlessProbe {
    fn new(cases: Vec<ProbeCase>) -> Self {
        let mut app = App::new();
        app.insert_resource(ProbeCases(cases))
            .init_resource::<ProbeResults>()
            .init_resource::<ProbeTargets>()
            .init_resource::<CaptureState>()
            .add_plugins(
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
                    // Match the native runtime's map sampler.
                    .set(ImagePlugin::default_nearest())
                    .disable::<WinitPlugin>(),
            )
            .add_systems(Startup, setup_probe_scene)
            .add_systems(Update, schedule_screenshots_after_warmup);
        app.finish();
        app.cleanup();
        Self(std::mem::take(app.sub_apps_mut()))
    }

    fn update(&mut self) {
        self.0.update();
        self.0
            .main
            .world()
            .resource::<RenderDevice>()
            .wgpu_device()
            .poll(PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .expect("GPU poll should complete screenshot readback");
    }

    fn result_count(&self) -> usize {
        self.0.main.world().resource::<ProbeResults>().frames.len()
    }

    fn results(&self) -> ProbeResults {
        self.0.main.world().resource::<ProbeResults>().clone()
    }
}

fn probe_cases() -> Vec<ProbeCase> {
    let mut cases = Vec::new();
    for gutter in [Gutter::Transparent, Gutter::EdgeExtruded] {
        for (fraction_label, fraction) in [("0", 0.0), ("0.25", 0.25), ("0.5", 0.5), ("0.75", 0.75)]
        {
            for (axis, offset) in [
                ("x", Vec2::new(fraction, 0.0)),
                ("y", Vec2::new(0.0, fraction)),
                ("diagonal", Vec2::splat(fraction)),
            ] {
                let label = format!("{axis}@{fraction_label}");
                for msaa in [Msaa::Sample4, Msaa::Off] {
                    cases.push(ProbeCase {
                        label: label.clone(),
                        offset,
                        msaa,
                        gutter,
                    });
                }
            }
        }
    }
    cases
}

fn setup_probe_scene(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
    mut targets: ResMut<ProbeTargets>,
    cases: Res<ProbeCases>,
) {
    let transparent_atlas = images.add(make_atlas(Gutter::Transparent));
    let extruded_atlas = images.add(make_atlas(Gutter::EdgeExtruded));
    let layout = layouts.add(make_atlas_layout());

    for (case_index, case) in cases.0.iter().enumerate() {
        let mut target = Image::new_target_texture(
            TARGET_WIDTH,
            TARGET_HEIGHT,
            TextureFormat::Rgba8UnormSrgb,
            None,
        );
        target.texture_descriptor.usage |= TextureUsages::COPY_SRC;
        let target = images.add(target);
        targets.0.push(target.clone());
        let layer = RenderLayers::layer(case_index);
        let atlas = match case.gutter {
            Gutter::Transparent => transparent_atlas.clone(),
            Gutter::EdgeExtruded => extruded_atlas.clone(),
        };

        commands.spawn((
            Camera2d,
            Camera {
                clear_color: ClearColorConfig::Custom(Color::srgba(0.0, 0.0, 0.0, 0.0)),
                ..default()
            },
            RenderTarget::Image(target.clone().into()),
            case.msaa,
            Transform::from_xyz(case.offset.x, case.offset.y, 0.0),
            layer.clone(),
        ));

        for (atlas_index, translation) in [
            (0, Vec3::new(-24.0, 16.0, 0.0)),
            (1, Vec3::new(24.0, 16.0, 0.0)),
            (2, Vec3::new(-24.0, -16.0, 0.0)),
            (3, Vec3::new(24.0, -16.0, 0.0)),
        ] {
            commands.spawn((
                Sprite {
                    image: atlas.clone(),
                    texture_atlas: Some(TextureAtlas {
                        layout: layout.clone(),
                        index: atlas_index,
                    }),
                    custom_size: Some(Vec2::new(TILE_WIDTH as f32, TILE_HEIGHT as f32)),
                    ..default()
                },
                Transform::from_translation(translation),
                layer.clone(),
            ));
        }
    }
}

fn schedule_screenshots_after_warmup(
    mut commands: Commands,
    targets: Res<ProbeTargets>,
    mut state: ResMut<CaptureState>,
) {
    if state.scheduled {
        return;
    }
    state.warmup_updates += 1;
    if state.warmup_updates < WARMUP_UPDATES {
        return;
    }
    state.scheduled = true;
    for (case_index, target) in targets.0.iter().cloned().enumerate() {
        commands.spawn(Screenshot::image(target)).observe(
            move |event: On<ScreenshotCaptured>, mut results: ResMut<ProbeResults>| {
                results.frames.insert(
                    case_index,
                    event
                        .image
                        .data
                        .as_ref()
                        .expect("screenshot readback should contain RGBA bytes")
                        .clone(),
                );
            },
        );
    }
}

fn make_atlas(gutter: Gutter) -> Image {
    let mut pixels = vec![0; (ATLAS_SIZE * ATLAS_SIZE * 4) as usize];
    for (index, rect) in ATLAS_RECTS.iter().copied().enumerate() {
        fill_rect(&mut pixels, rect, TILE_COLORS[index]);
        if gutter == Gutter::EdgeExtruded {
            extrude_tile_edge(&mut pixels, rect);
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: ATLAS_SIZE,
            height: ATLAS_SIZE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = bevy::image::ImageSampler::nearest();
    image
}

fn fill_rect(pixels: &mut [u8], rect: (u32, u32, u32, u32), color: [u8; 4]) {
    for y in rect.1..rect.3 {
        for x in rect.0..rect.2 {
            set_pixel(pixels, x, y, color);
        }
    }
}

fn extrude_tile_edge(pixels: &mut [u8], rect: (u32, u32, u32, u32)) {
    for y in (rect.1 - 1)..(rect.3 + 1) {
        for x in (rect.0 - 1)..(rect.2 + 1) {
            if (rect.0..rect.2).contains(&x) && (rect.1..rect.3).contains(&y) {
                continue;
            }
            let source_x = x.clamp(rect.0, rect.2 - 1);
            let source_y = y.clamp(rect.1, rect.3 - 1);
            set_pixel(pixels, x, y, atlas_pixel(pixels, source_x, source_y));
        }
    }
}

fn atlas_pixel(pixels: &[u8], x: u32, y: u32) -> [u8; 4] {
    let offset = ((y * ATLAS_SIZE + x) * 4) as usize;
    pixels[offset..offset + 4]
        .try_into()
        .expect("RGBA atlas pixel")
}

fn set_pixel(pixels: &mut [u8], x: u32, y: u32, color: [u8; 4]) {
    let offset = ((y * ATLAS_SIZE + x) * 4) as usize;
    pixels[offset..offset + 4].copy_from_slice(&color);
}

fn make_atlas_layout() -> TextureAtlasLayout {
    let mut layout = TextureAtlasLayout::new_empty(UVec2::splat(ATLAS_SIZE));
    for (min_x, min_y, max_x, max_y) in ATLAS_RECTS {
        layout.add_texture(URect {
            min: UVec2::new(min_x, min_y),
            max: UVec2::new(max_x, max_y),
        });
    }
    layout
}

struct FrameObservation {
    label: String,
    offset: Vec2,
    msaa: &'static str,
    gutter: &'static str,
    seam_transparent_pixels: usize,
    minimum_alpha: u8,
    interior_opaque_samples: usize,
    interior_colored_samples: usize,
}

impl FrameObservation {
    fn analyze(case: &ProbeCase, pixels: &[u8]) -> Self {
        assert_eq!(pixels.len(), (TARGET_WIDTH * TARGET_HEIGHT * 4) as usize);
        let mut seam_transparent_pixels = 0;
        let mut minimum_alpha = u8::MAX;
        for y in 3..TARGET_HEIGHT - 3 {
            for x in 3..TARGET_WIDTH - 3 {
                // The shared vertical edge is x=48; the shared horizontal edge is y=32.
                if !(45..=50).contains(&x) && !(29..=34).contains(&y) {
                    continue;
                }
                let alpha = target_pixel(pixels, x, y)[3];
                minimum_alpha = minimum_alpha.min(alpha);
                seam_transparent_pixels += usize::from(alpha < u8::MAX);
            }
        }
        let interior_samples = [(24, 16), (72, 16), (24, 48), (72, 48)];
        let interior_opaque_samples = interior_samples
            .iter()
            .filter(|&&(x, y)| target_pixel(pixels, x, y)[3] == u8::MAX)
            .count();
        let interior_colored_samples = interior_samples
            .iter()
            .filter(|&&(x, y)| {
                let rgba = target_pixel(pixels, x, y);
                rgba[3] == u8::MAX && rgba[..3].iter().copied().max().unwrap_or(0) > 16
            })
            .count();
        Self {
            label: case.label.clone(),
            offset: case.offset,
            msaa: msaa_name(case.msaa),
            gutter: case.gutter.name(),
            seam_transparent_pixels,
            minimum_alpha,
            interior_opaque_samples,
            interior_colored_samples,
        }
    }

    fn has_opaque_interior(&self) -> bool {
        self.interior_opaque_samples == 4 && self.interior_colored_samples == 4
    }

    fn zero_seam(&self) -> bool {
        self.seam_transparent_pixels == 0 && self.minimum_alpha == u8::MAX
    }
}

fn target_pixel(pixels: &[u8], x: u32, y: u32) -> [u8; 4] {
    let offset = ((y * TARGET_WIDTH + x) * 4) as usize;
    pixels[offset..offset + 4]
        .try_into()
        .expect("RGBA target pixel")
}

fn msaa_name(msaa: Msaa) -> &'static str {
    match msaa {
        Msaa::Off => "Off",
        Msaa::Sample4 => "Sample4",
        Msaa::Sample2 => "Sample2",
        Msaa::Sample8 => "Sample8",
    }
}

fn zero_seam_configurations(observations: &[FrameObservation]) -> Vec<String> {
    ["transparent", "edgeExtruded"]
        .into_iter()
        .flat_map(|gutter| {
            ["Sample4", "Off"].into_iter().filter_map(move |msaa| {
                let matching: Vec<_> = observations
                    .iter()
                    .filter(|observation| observation.gutter == gutter && observation.msaa == msaa)
                    .collect();
                (!matching.is_empty() && matching.iter().all(|observation| observation.zero_seam()))
                    .then(|| format!("{gutter}+{msaa}"))
            })
        })
        .collect()
}

fn render_timeout_json(expected_frames: usize, captured_frames: usize) -> String {
    format!(
        "{{\n  \"status\": \"inconclusive\",\n  \"outcome\": \"screenshot_readback_timed_out\",\n  \"warmupUpdates\": {WARMUP_UPDATES},\n  \"expectedFrames\": {expected_frames},\n  \"capturedFrames\": {captured_frames}\n}}"
    )
}

fn render_observations_json(
    observations: &[FrameObservation],
    status: &str,
    outcome: &str,
    zero_seam_configurations: &[String],
) -> String {
    let mut report = format!(
        "{{\n  \"status\": \"{status}\",\n  \"outcome\": \"{outcome}\",\n  \"warmupUpdates\": {WARMUP_UPDATES},\n  \"zeroSeamConfigurations\": [{}],\n  \"frames\": [\n",
        zero_seam_configurations
            .iter()
            .map(|name| format!("\"{name}\""))
            .collect::<Vec<_>>()
            .join(", ")
    );
    for (index, observation) in observations.iter().enumerate() {
        let suffix = if index + 1 == observations.len() {
            ""
        } else {
            ","
        };
        report.push_str(&format!(
            "    {{\"case\":\"{}\",\"cameraOffset\":[{:.2},{:.2}],\"gutter\":\"{}\",\"msaa\":\"{}\",\"seamTransparentPixels\":{},\"minimumAlpha\":{},\"interiorOpaqueSamples\":{},\"interiorColoredSamples\":{}}}{suffix}\n",
            observation.label,
            observation.offset.x,
            observation.offset.y,
            observation.gutter,
            observation.msaa,
            observation.seam_transparent_pixels,
            observation.minimum_alpha,
            observation.interior_opaque_samples,
            observation.interior_colored_samples,
        ));
    }
    report.push_str("  ]\n}");
    report
}
