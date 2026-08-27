//! Native Windows screenshot plugin for the 1024x768 native Bevy slice.
//!
//! The plugin is intentionally off by default. It only enables capture when
//! `MIR2_NATIVE_CAPTURE_DIR` is provided with a valid, non-empty value.

mod provenance;

use bevy::{
    input::ButtonInput,
    prelude::{
        App, Image, KeyCode, On, Plugin, Query, Res, ResMut, Resource, Update, Window, With,
    },
    render::view::screenshot::{Screenshot, ScreenshotCaptured},
    window::PrimaryWindow,
};
use mir2_client_bevy::crystal_ui::NativePlayerUiState;
use mir2_client_bevy::entities::{EntityKind, EntityModelSet};
use mir2_client_bevy::native_shell::{NativeShellModel, NativeShellScreen};
use mir2_client_bevy::quest_model::{CombatTargetModel, QuestStatus, QuestTracker};
use mir2_client_bevy::read_model::UiReadModel;
use serde::Serialize;
use std::{
    env,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

const ENV_CAPTURE_DIR: &str = "MIR2_NATIVE_CAPTURE_DIR";
const ENV_CAPTURE_LABEL: &str = "MIR2_NATIVE_CAPTURE_LABEL";
const ENV_CAPTURE_AUTO_SCREEN: &str = "MIR2_NATIVE_CAPTURE_AUTO_SCREEN";
const ENV_CAPTURE_QUEST_INDEX: &str = "MIR2_NATIVE_CAPTURE_QUEST_INDEX";
const ENV_CAPTURE_RUN_ID: &str = "MIR2_NATIVE_CAPTURE_RUN_ID";
const AUTO_CAPTURE_WAIT_FRAMES: u32 = 60;
const DEFAULT_STABLE_SLUG: &str = "unknown";
const DEFAULT_LABEL_PREFIX: &str = "mir2";
const CAPTURE_SCHEMA_VERSION: &str = "mir2-native-visual-capture-v1";
const CAPTURE_DRAFT_SCHEMA_VERSION: &str = "mir2-native-visual-capture-draft-v1";
const CAPTURE_PRODUCER: &str = "windows-native";
static CAPTURE_TRACE_FRAME: AtomicU64 = AtomicU64::new(0);
static CAPTURE_TEMP_INDEX: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Resource)]
pub struct NativeCaptureConfig {
    capture_dir: PathBuf,
    capture_label: Option<String>,
    auto_target: Option<NativeCaptureTarget>,
    quest_index: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeCaptureTarget {
    Screen(NativeShellScreen),
    QuestAccepted,
    Combat,
    QuestComplete,
}

impl NativeCaptureConfig {
    pub fn from_env() -> Option<Self> {
        let capture_dir = env::var(ENV_CAPTURE_DIR).ok();
        let label = env::var(ENV_CAPTURE_LABEL).ok();
        let auto = env::var(ENV_CAPTURE_AUTO_SCREEN).ok();
        let quest_index = env::var(ENV_CAPTURE_QUEST_INDEX).ok();
        Self::from_values(
            capture_dir.as_deref(),
            label.as_deref(),
            auto.as_deref(),
            quest_index.as_deref(),
        )
    }

    pub fn from_values(
        capture_dir: Option<&str>,
        capture_label: Option<&str>,
        auto_screen: Option<&str>,
        quest_index: Option<&str>,
    ) -> Option<Self> {
        let capture_dir = sanitize_capture_dir(capture_dir)?;
        let capture_label = capture_label.and_then(|value| {
            let label = sanitize_capture_label(value);
            (!label.is_empty()).then_some(label)
        });
        let auto_target = auto_screen.and_then(parse_capture_target_slug);
        let quest_index = quest_index
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .and_then(|value| value.parse::<i32>().ok())
            .filter(|value| *value >= 0);
        Some(Self {
            capture_dir,
            capture_label,
            auto_target,
            quest_index,
        })
    }

    fn label_prefix(&self) -> &str {
        self.capture_label
            .as_deref()
            .filter(|value| !value.is_empty())
            .unwrap_or(DEFAULT_LABEL_PREFIX)
    }

    pub fn filename_prefix(&self, screen_slug: &str) -> String {
        format!(
            "{}-{}-{}",
            self.label_prefix(),
            sanitize_capture_label(screen_slug),
            unix_time_millis()
        )
    }
}

#[derive(Debug, Default, Resource)]
struct NativeCaptureRuntime {
    capture_index: u64,
    auto: Option<NativeAutoCaptureState>,
}

#[derive(Debug)]
struct NativeAutoCaptureState {
    target: NativeCaptureTarget,
    countdown: Option<u32>,
    done: bool,
}

/// Snapshot taken when a screenshot is requested, before Bevy completes the
/// asynchronous GPU readback. This prevents the JSON sidecar from describing a
/// later shell/map/UI state than the PNG itself requested.
#[derive(Debug, Clone)]
struct NativeCaptureRequest {
    png_path: PathBuf,
    sidecar_path: PathBuf,
    captured_at_ms: u128,
    run_id: Option<String>,
    scene: String,
    dpi_scale: Option<f32>,
    ui_state: Option<String>,
    world: NativeCaptureWorld,
    build: NativeCaptureBuild,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeCaptureWorld {
    map: Option<String>,
    x: Option<i32>,
    y: Option<i32>,
    // Lighting is managed by the Windows host bridge and is not exposed as a
    // capture resource. A missing light must remain explicit rather than be
    // guessed from time, map, or a visual effect.
    light: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeCaptureBuild {
    // Populated only when the package provenance module cross-checks the
    // running executable against VERSION.json and PACKAGE-MANIFEST.json.
    source_revision: Option<String>,
    executable_sha256: Option<String>,
    asset_manifest_sha256: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeCaptureDraftSidecar {
    schema_version: &'static str,
    producer: &'static str,
    run_id: Option<String>,
    scene: String,
    captured_at: String,
    image_path: String,
    image_sha256: String,
    logical_size: NativeCaptureSize,
    dpi_scale: Option<f32>,
    ui_state: Option<String>,
    world: NativeCaptureWorld,
    build: NativeCaptureBuild,
    acceptance: NativeCaptureAcceptance,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeCaptureV1Sidecar {
    schema_version: &'static str,
    producer: &'static str,
    run_id: String,
    scene: String,
    captured_at: String,
    image_path: String,
    image_sha256: String,
    logical_size: NativeCaptureSize,
    dpi_scale: f32,
    ui_state: String,
    world: Option<NativeCaptureV1World>,
    build: NativeCaptureV1Build,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeCaptureV1World {
    map: String,
    x: i32,
    y: i32,
    light: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeCaptureV1Build {
    source_revision: String,
    executable_sha256: String,
    asset_manifest_sha256: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeCaptureSize {
    width: u32,
    height: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeCaptureAcceptance {
    eligible: bool,
    blockers: Vec<&'static str>,
}

impl NativeCaptureRuntime {
    fn next_index(&mut self) -> u64 {
        self.capture_index += 1;
        self.capture_index
    }

    fn auto_capture_path(&mut self, config: &NativeCaptureConfig, screen_slug: &str) -> PathBuf {
        self.capture_dir_path(config, screen_slug)
    }

    fn capture_dir_path(&mut self, config: &NativeCaptureConfig, screen_slug: &str) -> PathBuf {
        let filename = format!(
            "{}-{}.png",
            config.filename_prefix(screen_slug),
            self.next_index()
        );
        config.capture_dir.join(filename)
    }
}

#[derive(Debug, Clone)]
pub struct Mir2NativeScreenshotPlugin;

impl Plugin for Mir2NativeScreenshotPlugin {
    fn build(&self, app: &mut App) {
        let Some(config) = NativeCaptureConfig::from_env() else {
            return;
        };

        if std::fs::create_dir_all(&config.capture_dir).is_err() {
            return;
        }

        let mut runtime = NativeCaptureRuntime::default();
        if let Some(target) = config.auto_target {
            runtime.auto = Some(NativeAutoCaptureState {
                target,
                countdown: None,
                done: false,
            });
        }

        app.insert_resource(config);
        app.insert_resource(runtime);
        app.add_systems(Update, (manual_capture_system, auto_capture_system));
    }
}

fn manual_capture_system(
    mut commands: bevy::prelude::Commands,
    keys: Res<ButtonInput<KeyCode>>,
    config: Res<NativeCaptureConfig>,
    mut runtime: ResMut<NativeCaptureRuntime>,
    shell: Option<Res<NativeShellModel>>,
    ui: Option<Res<UiReadModel>>,
    entities: Option<Res<EntityModelSet>>,
    player_ui: Option<Res<NativePlayerUiState>>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    if !keys.just_pressed(KeyCode::F12) {
        return;
    }

    let screen_slug = shell
        .as_deref()
        .map(|model| native_shell_screen_slug(model.screen))
        .unwrap_or(DEFAULT_STABLE_SLUG);
    let path = runtime.auto_capture_path(&config, screen_slug);
    queue_atomic_capture(
        &mut commands,
        capture_request(
            path,
            screen_slug,
            shell.as_deref(),
            ui.as_deref(),
            entities.as_deref(),
            player_ui.as_deref(),
            primary_dpi_scale(&windows),
        ),
    );
}

fn auto_capture_system(
    mut commands: bevy::prelude::Commands,
    config: Res<NativeCaptureConfig>,
    mut runtime: ResMut<NativeCaptureRuntime>,
    shell: Option<Res<NativeShellModel>>,
    tracker: Option<Res<QuestTracker>>,
    combat: Option<Res<CombatTargetModel>>,
    ui: Option<Res<UiReadModel>>,
    entities: Option<Res<EntityModelSet>>,
    player_ui: Option<Res<NativePlayerUiState>>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    let Some(shell) = shell.as_deref() else {
        return;
    };
    let capture_slug = {
        let Some(auto) = runtime.auto.as_mut() else {
            return;
        };
        if auto.done {
            return;
        }
        let target_matches = capture_target_matches(
            auto.target,
            shell,
            tracker.as_deref(),
            combat.as_deref(),
            config.quest_index,
        );
        if env::var_os("MIR2_NATIVE_TRACE_CAPTURE").is_some()
            && CAPTURE_TRACE_FRAME.fetch_add(1, Ordering::Relaxed) % 60 == 0
        {
            let quests = tracker
                .as_deref()
                .map(|tracker| {
                    tracker
                        .active_quests
                        .iter()
                        .map(|quest| (quest.quest_index, quest.status.clone()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let combat_target = combat.as_deref().and_then(|model| {
                model.target.as_ref().map(|target| {
                    (
                        target.object_id,
                        target.name.clone(),
                        target.hp,
                        target.max_hp,
                    )
                })
            });
            eprintln!(
                "[native-capture] target={:?} shell={:?} quest_index={:?} quests={:?} combat={:?} matches={target_matches}",
                auto.target, shell.screen, config.quest_index, quests, combat_target
            );
        }
        if !target_matches {
            auto.countdown = None;
            return;
        }
        match auto.countdown {
            None => {
                auto.countdown = Some(AUTO_CAPTURE_WAIT_FRAMES);
                None
            }
            Some(0) => {
                auto.done = true;
                auto.countdown = None;
                Some(capture_target_slug(auto.target))
            }
            Some(current) => {
                auto.countdown = current.checked_sub(1);
                None
            }
        }
    };
    if let Some(slug) = capture_slug {
        let path = runtime.capture_dir_path(&config, slug);
        queue_atomic_capture(
            &mut commands,
            capture_request(
                path,
                slug,
                Some(shell),
                ui.as_deref(),
                entities.as_deref(),
                player_ui.as_deref(),
                primary_dpi_scale(&windows),
            ),
        );
    }
}

fn queue_atomic_capture(commands: &mut bevy::prelude::Commands, request: NativeCaptureRequest) {
    commands.spawn(Screenshot::primary_window()).observe(
        move |captured: On<ScreenshotCaptured>| {
            if let Err(error) = write_capture_and_sidecar(&captured.image, &request) {
                eprintln!(
                    "[native-capture] failed to write {}: {error}",
                    request.png_path.display()
                );
            }
        },
    );
}

fn capture_request(
    png_path: PathBuf,
    scene: &str,
    shell: Option<&NativeShellModel>,
    ui: Option<&UiReadModel>,
    entities: Option<&EntityModelSet>,
    player_ui: Option<&NativePlayerUiState>,
    dpi_scale: Option<f32>,
) -> NativeCaptureRequest {
    let is_world_scene = matches!(
        scene,
        "in-game" | "quest-accepted" | "combat" | "quest-complete"
    );
    let map_name = is_world_scene
        .then(|| ui.and_then(|model| model.player.map_name.clone()))
        .flatten();
    let world = NativeCaptureWorld {
        light: map_name
            .as_deref()
            .and_then(crate::map_parser::lighting::capture_light_state_for_map),
        map: map_name,
        x: is_world_scene
            .then(|| self_player_position(entities).map(|position| position.0))
            .flatten(),
        y: is_world_scene
            .then(|| self_player_position(entities).map(|position| position.1))
            .flatten(),
    };
    let ui_state = safe_ui_state_slug(shell, player_ui);
    let sidecar_path = png_path.with_extension("json");

    NativeCaptureRequest {
        png_path,
        sidecar_path,
        captured_at_ms: unix_time_millis(),
        run_id: env::var(ENV_CAPTURE_RUN_ID)
            .ok()
            .and_then(|value| sanitize_capture_run_id(&value)),
        scene: scene.to_owned(),
        dpi_scale,
        ui_state,
        world,
        build: provenance::trusted_build_provenance(),
    }
}

fn self_player_position(entities: Option<&EntityModelSet>) -> Option<(i32, i32)> {
    entities?
        .entities
        .iter()
        .find(|entity| entity.kind == EntityKind::SelfPlayer)
        .map(|entity| (entity.x, entity.y))
}

fn primary_dpi_scale(windows: &Query<&Window, With<PrimaryWindow>>) -> Option<f32> {
    windows
        .single()
        .ok()
        .map(Window::scale_factor)
        .filter(|value| value.is_finite() && (0.5..=4.0).contains(value))
}

/// This is intentionally a fixed visibility-only summary. In particular, it
/// excludes `login_account`, `login_password`, chat and guild drafts, and any
/// user-authored/private text in `NativePlayerUiState`.
fn safe_ui_state_slug(
    shell: Option<&NativeShellModel>,
    player_ui: Option<&NativePlayerUiState>,
) -> Option<String> {
    let shell_slug = shell
        .map(|model| native_shell_screen_slug(model.screen))
        .unwrap_or(DEFAULT_STABLE_SLUG);
    let Some(player_ui) = player_ui else {
        return shell.map(|_| format!("shell={shell_slug}"));
    };
    let core = &player_ui.core;
    Some(format!(
        "shell={};screen={:?};panel={:?};minimap={};chatFocused={};security={:?};inspect={};inventoryOperation={};dropConfirm={}",
        shell_slug,
        core.screen,
        core.panel,
        core.minimap_visible,
        core.chat_focused,
        core.security.panel,
        player_ui.inspect.is_some(),
        player_ui.inventory_operation.is_some(),
        player_ui.drop_confirmation.is_some(),
    ))
}

fn write_capture_and_sidecar(image: &Image, request: &NativeCaptureRequest) -> io::Result<()> {
    if request.sidecar_path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "refusing to pair with an existing sidecar: {}",
                request.sidecar_path.display()
            ),
        ));
    }
    let rgba = image
        .clone()
        .try_into_dynamic()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?
        .to_rgba8();
    let (width, height) = rgba.dimensions();
    write_png_atomically(&request.png_path, width, height, rgba.as_raw())?;

    let image_sha256 = sha256_hex(&fs::read(&request.png_path)?);
    let json = serialize_capture_sidecar(request, width, height, image_sha256)?;
    // The sidecar is the capture-completion marker. It is deliberately
    // attempted only after the final PNG exists and hashes successfully; its
    // schema still decides whether it is acceptance-eligible or only a draft.
    write_atomic_bytes(&request.sidecar_path, &json)
}

fn serialize_capture_sidecar(
    request: &NativeCaptureRequest,
    width: u32,
    height: u32,
    image_sha256: String,
) -> io::Result<Vec<u8>> {
    if let Some(sidecar) = native_capture_v1_sidecar(request, width, height, image_sha256.clone()) {
        return serde_json::to_vec_pretty(&sidecar)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()));
    }
    let sidecar = NativeCaptureDraftSidecar {
        schema_version: CAPTURE_DRAFT_SCHEMA_VERSION,
        producer: CAPTURE_PRODUCER,
        run_id: request.run_id.clone(),
        scene: request.scene.clone(),
        captured_at: format_rfc3339_utc(request.captured_at_ms),
        image_path: request
            .png_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_owned(),
        image_sha256,
        logical_size: NativeCaptureSize { width, height },
        dpi_scale: request.dpi_scale,
        ui_state: request.ui_state.clone(),
        world: request.world.clone(),
        build: request.build.clone(),
        // The visual-pair verifier intentionally rejects this draft sidecar:
        // lighting, DPI, complete UI state, and trusted build provenance must
        // be injected by host integration before a capture can be accepted.
        acceptance: NativeCaptureAcceptance {
            eligible: false,
            blockers: capture_acceptance_blockers(request, width, height),
        },
    };
    serde_json::to_vec_pretty(&sidecar)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))
}

fn native_capture_v1_sidecar(
    request: &NativeCaptureRequest,
    width: u32,
    height: u32,
    image_sha256: String,
) -> Option<NativeCaptureV1Sidecar> {
    if !capture_acceptance_blockers(request, width, height).is_empty() {
        return None;
    }
    let world = if is_world_scene(&request.scene) {
        Some(NativeCaptureV1World {
            map: request.world.map.clone()?,
            x: request.world.x?,
            y: request.world.y?,
            light: request.world.light.clone()?,
        })
    } else {
        None
    };
    Some(NativeCaptureV1Sidecar {
        schema_version: CAPTURE_SCHEMA_VERSION,
        producer: CAPTURE_PRODUCER,
        run_id: request.run_id.clone()?,
        scene: request.scene.clone(),
        captured_at: format_rfc3339_utc(request.captured_at_ms),
        image_path: request
            .png_path
            .file_name()
            .and_then(|value| value.to_str())?
            .to_owned(),
        image_sha256,
        logical_size: NativeCaptureSize { width, height },
        dpi_scale: request.dpi_scale?,
        ui_state: request.ui_state.clone()?,
        world,
        build: NativeCaptureV1Build {
            source_revision: request.build.source_revision.clone()?,
            executable_sha256: request.build.executable_sha256.clone()?,
            asset_manifest_sha256: request.build.asset_manifest_sha256.clone()?,
        },
    })
}

fn capture_acceptance_blockers(
    request: &NativeCaptureRequest,
    width: u32,
    height: u32,
) -> Vec<&'static str> {
    let mut blockers = Vec::new();
    if request.run_id.is_none() {
        blockers.push("run-id-unavailable-or-invalid");
    }
    if !matches!(
        request.scene.as_str(),
        "login" | "character-select" | "in-game" | "quest-accepted" | "combat" | "quest-complete"
    ) {
        blockers.push("scene-not-supported-by-v1");
    }
    if width != 1024 || height != 768 {
        blockers.push("logical-size-not-1024x768");
    }
    if request
        .dpi_scale
        .is_none_or(|value| !value.is_finite() || !(0.5..=4.0).contains(&value))
    {
        blockers.push("dpi-scale-unavailable");
    }
    if request.ui_state.is_none() {
        blockers.push("safe-ui-state-unavailable");
    }
    if is_world_scene(&request.scene)
        && (request.world.map.is_none()
            || request.world.x.is_none()
            || request.world.y.is_none()
            || request.world.light.is_none())
    {
        blockers.push("authoritative-world-state-incomplete");
    }
    if request
        .build
        .source_revision
        .as_deref()
        .is_none_or(|value| value.is_empty() || value.len() > 160)
        || request
            .build
            .executable_sha256
            .as_deref()
            .is_none_or(|value| !is_sha256_hex(value))
        || request
            .build
            .asset_manifest_sha256
            .as_deref()
            .is_none_or(|value| !is_sha256_hex(value))
    {
        blockers.push("trusted-build-provenance-unavailable");
    }
    blockers
}

fn is_world_scene(scene: &str) -> bool {
    matches!(
        scene,
        "in-game" | "quest-accepted" | "combat" | "quest-complete"
    )
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn write_png_atomically(path: &Path, width: u32, height: u32, rgba: &[u8]) -> io::Result<()> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
        writer
            .write_image_data(rgba)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
        writer
            .finish()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    }
    write_atomic_bytes(path, &bytes)
}

fn write_atomic_bytes(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let temporary_path = temporary_capture_path(path);
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        if path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("capture path already exists: {}", path.display()),
            ));
        }
        fs::rename(&temporary_path, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    result
}

fn temporary_capture_path(path: &Path) -> PathBuf {
    let index = CAPTURE_TEMP_INDEX.fetch_add(1, Ordering::Relaxed);
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("capture");
    path.with_file_name(format!(".{name}.{index}.tmp"))
}

fn capture_target_matches(
    target: NativeCaptureTarget,
    shell: &NativeShellModel,
    tracker: Option<&QuestTracker>,
    combat: Option<&CombatTargetModel>,
    quest_index: Option<i32>,
) -> bool {
    match target {
        NativeCaptureTarget::Screen(screen) => shell.screen == screen,
        NativeCaptureTarget::QuestAccepted => {
            shell.screen == NativeShellScreen::InGame
                && tracker.is_some_and(|tracker| {
                    tracker.active_quests.iter().any(|quest| {
                        quest_index.is_none_or(|index| quest.quest_index == index)
                            && quest.status == QuestStatus::InProgress
                    })
                })
        }
        NativeCaptureTarget::Combat => {
            shell.screen == NativeShellScreen::InGame
                && combat
                    .and_then(|model| model.target.as_ref())
                    .is_some_and(|target| {
                        target.max_hp > 0 && target.hp > 0 && target.hp < target.max_hp
                    })
        }
        NativeCaptureTarget::QuestComplete => {
            shell.screen == NativeShellScreen::InGame
                && tracker.is_some_and(|tracker| {
                    tracker.active_quests.iter().any(|quest| {
                        quest_index.is_none_or(|index| quest.quest_index == index)
                            && quest.status == QuestStatus::Completed
                    })
                })
        }
    }
}

fn capture_target_slug(target: NativeCaptureTarget) -> &'static str {
    match target {
        NativeCaptureTarget::Screen(screen) => native_shell_screen_slug(screen),
        NativeCaptureTarget::QuestAccepted => "quest-accepted",
        NativeCaptureTarget::Combat => "combat",
        NativeCaptureTarget::QuestComplete => "quest-complete",
    }
}

fn native_shell_screen_slug(screen: NativeShellScreen) -> &'static str {
    match screen {
        NativeShellScreen::Connecting => "connecting",
        NativeShellScreen::Login => "login",
        NativeShellScreen::Authenticating => "authenticating",
        NativeShellScreen::CharacterSelect => "character-select",
        NativeShellScreen::CharacterCreate => "character-create",
        NativeShellScreen::StartingGame => "starting-game",
        NativeShellScreen::InGame => "in-game",
        NativeShellScreen::ConnectionLost => "connection-lost",
        NativeShellScreen::ChangePassword => "change-password",
        NativeShellScreen::SafeKey => "safe-key",
        NativeShellScreen::DeleteConfirm { .. } => "delete-confirm",
    }
}

fn parse_shell_screen_slug(raw: &str) -> Option<NativeShellScreen> {
    let normalized = sanitize_capture_label(&raw.to_ascii_lowercase().replace('_', "-"));
    match normalized.as_str() {
        "connecting" => Some(NativeShellScreen::Connecting),
        "login" => Some(NativeShellScreen::Login),
        "authenticating" => Some(NativeShellScreen::Authenticating),
        "characterselect" => Some(NativeShellScreen::CharacterSelect),
        "character-select" | "character_select" => Some(NativeShellScreen::CharacterSelect),
        "charactercreate" => Some(NativeShellScreen::CharacterCreate),
        "character-create" | "character_create" => Some(NativeShellScreen::CharacterCreate),
        "startinggame" => Some(NativeShellScreen::StartingGame),
        "starting-game" | "starting_game" => Some(NativeShellScreen::StartingGame),
        "in game" | "in-game" | "in_game" | "ingame" => Some(NativeShellScreen::InGame),
        "connection-lost" | "connectionlost" | "disconnected" => {
            Some(NativeShellScreen::ConnectionLost)
        }
        _ => None,
    }
}

fn parse_capture_target_slug(raw: &str) -> Option<NativeCaptureTarget> {
    let normalized = sanitize_capture_label(&raw.to_ascii_lowercase().replace('_', "-"));
    match normalized.as_str() {
        "quest-accepted" | "questaccepted" => Some(NativeCaptureTarget::QuestAccepted),
        "combat" | "combat-damaged" => Some(NativeCaptureTarget::Combat),
        "quest-complete" | "questcomplete" | "quest-completed" => {
            Some(NativeCaptureTarget::QuestComplete)
        }
        _ => parse_shell_screen_slug(raw).map(NativeCaptureTarget::Screen),
    }
}

fn sanitize_capture_dir(raw: Option<&str>) -> Option<PathBuf> {
    let raw = raw?.trim();
    if raw.is_empty() || raw.contains('\0') {
        return None;
    }

    let path = Path::new(raw);
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return None;
    }

    Some(path.to_path_buf())
}

fn sanitize_capture_label(raw: &str) -> String {
    let mut normalized = String::with_capacity(raw.len());
    let mut last_separator = false;

    for c in raw.chars() {
        let c = c.to_ascii_lowercase();
        let safe = match c {
            'a'..='z' | '0'..='9' | '-' | '_' | '.' => true,
            ' ' => true,
            _ => false,
        };
        if safe {
            if c == ' ' {
                if !last_separator && !normalized.is_empty() {
                    normalized.push('-');
                    last_separator = true;
                }
            } else {
                normalized.push(c);
                last_separator = false;
            }
        } else if !last_separator && !normalized.is_empty() {
            normalized.push('-');
            last_separator = true;
        }
    }

    let sanitized = normalized.trim_matches(|character| matches!(character, '-' | '_' | '.'));
    if sanitized.is_empty() {
        DEFAULT_LABEL_PREFIX.to_string()
    } else {
        sanitized.to_string()
    }
}

fn sanitize_capture_run_id(raw: &str) -> Option<String> {
    let value = raw.trim();
    if value.is_empty() || value.len() > 96 {
        return None;
    }
    let mut chars = value.chars();
    if !chars.next()?.is_ascii_alphanumeric()
        || !chars.all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
    {
        return None;
    }
    Some(value.to_owned())
}

fn unix_time_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |now| now.as_millis())
}

fn format_rfc3339_utc(timestamp_ms: u128) -> String {
    let seconds = (timestamp_ms / 1_000).min(i64::MAX as u128) as i64;
    let milliseconds = timestamp_ms % 1_000;
    let days = seconds.div_euclid(86_400);
    let seconds_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_date_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{milliseconds:03}Z")
}

// Howard Hinnant's civil-from-days conversion, with day zero at 1970-01-01.
fn civil_date_from_days(days_since_unix_epoch: i64) -> (i64, u32, u32) {
    let z = days_since_unix_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let day_of_year = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    (
        year + if month <= 2 { 1 } else { 0 },
        month as u32,
        day as u32,
    )
}

fn sha256_hex(bytes: &[u8]) -> String {
    const INITIAL: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    let mut state = INITIAL;
    let bit_length = (bytes.len() as u128).saturating_mul(8);
    let mut padded = bytes.to_vec();
    padded.push(0x80);
    while (padded.len() + 8) % 64 != 0 {
        padded.push(0);
    }
    padded.extend_from_slice(&(bit_length as u64).to_be_bytes());

    for chunk in padded.chunks_exact(64) {
        let mut words = [0u32; 64];
        for (index, word) in words.iter_mut().take(16).enumerate() {
            *word =
                u32::from_be_bytes(chunk[index * 4..index * 4 + 4].try_into().unwrap_or([0; 4]));
        }
        for index in 16..64 {
            let s0 = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let s1 = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(s0)
                .wrapping_add(words[index - 7])
                .wrapping_add(s1);
        }

        let mut working = state;
        for (index, constant) in K.iter().enumerate() {
            let sigma1 = working[4].rotate_right(6)
                ^ working[4].rotate_right(11)
                ^ working[4].rotate_right(25);
            let choose = (working[4] & working[5]) ^ (!working[4] & working[6]);
            let temp1 = working[7]
                .wrapping_add(sigma1)
                .wrapping_add(choose)
                .wrapping_add(*constant)
                .wrapping_add(words[index]);
            let sigma0 = working[0].rotate_right(2)
                ^ working[0].rotate_right(13)
                ^ working[0].rotate_right(22);
            let majority =
                (working[0] & working[1]) ^ (working[0] & working[2]) ^ (working[1] & working[2]);
            let temp2 = sigma0.wrapping_add(majority);
            working = [
                temp1.wrapping_add(temp2),
                working[0],
                working[1],
                working[2],
                working[3].wrapping_add(temp1),
                working[4],
                working[5],
                working[6],
            ];
        }
        for (value, addition) in state.iter_mut().zip(working) {
            *value = value.wrapping_add(addition);
        }
    }

    state.iter().map(|word| format!("{word:08x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_disabled_without_dir_env_value() {
        assert!(
            NativeCaptureConfig::from_values(None, Some("login"), Some("login"), None).is_none()
        );
        assert!(
            NativeCaptureConfig::from_values(Some(""), Some("login"), Some("login"), None)
                .is_none()
        );
        assert!(
            NativeCaptureConfig::from_values(Some("   "), Some("login"), Some("login"), None)
                .is_none()
        );
    }

    #[test]
    fn parse_shell_screen_slug_supports_stable_variants() {
        assert_eq!(
            parse_shell_screen_slug("login"),
            Some(NativeShellScreen::Login)
        );
        assert_eq!(
            parse_shell_screen_slug("character-select"),
            Some(NativeShellScreen::CharacterSelect)
        );
        assert_eq!(
            parse_shell_screen_slug("in-game"),
            Some(NativeShellScreen::InGame)
        );
        assert_eq!(
            parse_shell_screen_slug("starting_game"),
            Some(NativeShellScreen::StartingGame)
        );
        assert_eq!(parse_shell_screen_slug("bogus"), None);
        assert_eq!(
            parse_capture_target_slug("quest-accepted"),
            Some(NativeCaptureTarget::QuestAccepted)
        );
        assert_eq!(
            parse_capture_target_slug("combat"),
            Some(NativeCaptureTarget::Combat)
        );
        assert_eq!(
            parse_capture_target_slug("quest_complete"),
            Some(NativeCaptureTarget::QuestComplete)
        );
    }

    #[test]
    fn sanitize_capture_label_is_path_traversal_safe() {
        assert_eq!(sanitize_capture_label("../secret\\token"), "secret-token");
        assert_eq!(sanitize_capture_label("Login/Screen"), "login-screen");
        assert_eq!(sanitize_capture_label("  in@@game  "), "in-game");
        assert_eq!(sanitize_capture_label("..."), DEFAULT_LABEL_PREFIX);
    }

    #[test]
    fn sanitize_capture_dir_rejects_parent_component() {
        assert!(sanitize_capture_dir(Some("../captures")).is_none());
        assert!(sanitize_capture_dir(Some("a/../captures")).is_none());
    }

    #[test]
    fn from_values_validates_and_normalizes() {
        let cfg = NativeCaptureConfig::from_values(
            Some("C:/tmp/mir2-captures"),
            Some(" Login/Screen "),
            Some("in-game"),
            Some("2"),
        )
        .expect("config");
        assert_eq!(cfg.capture_label.as_deref(), Some("login-screen"));
        assert_eq!(
            cfg.auto_target,
            Some(NativeCaptureTarget::Screen(NativeShellScreen::InGame))
        );
        assert_eq!(cfg.quest_index, Some(2));
        assert_eq!(cfg.capture_dir.to_string_lossy(), "C:/tmp/mir2-captures");
    }

    #[test]
    fn capture_path_is_unique_and_deterministic() {
        let cfg =
            NativeCaptureConfig::from_values(Some("captures"), Some("Native Slice"), None, None)
                .expect("cfg");
        let mut runtime = NativeCaptureRuntime::default();
        let first = runtime.capture_dir_path(&cfg, "login");
        let second = runtime.capture_dir_path(&cfg, "login");

        assert_ne!(first, second);
        assert!(first
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("")
            .starts_with("native-slice-login-"));
        assert!(second
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("")
            .contains("login"));
    }

    #[test]
    fn runtime_auto_state_transitions_without_mutating_shell() {
        let mut runtime = NativeCaptureRuntime::default();
        runtime.auto = Some(NativeAutoCaptureState {
            target: NativeCaptureTarget::Screen(NativeShellScreen::InGame),
            countdown: Some(2),
            done: false,
        });
        assert_eq!(
            runtime.auto.as_ref().expect("auto").target,
            NativeCaptureTarget::Screen(NativeShellScreen::InGame)
        );
    }

    #[test]
    fn gameplay_capture_targets_require_authoritative_quest_and_health_states() {
        let shell = NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..Default::default()
        };
        let mut tracker = QuestTracker {
            active_quests: vec![mir2_client_bevy::quest_model::Quest {
                quest_index: 2,
                accept_npc_index: Some(4),
                finish_npc_index: Some(3),
                title: "CraftsLady's Request".to_owned(),
                npc_name: Some("CraftsLady".to_owned()),
                status: QuestStatus::InProgress,
                objectives: Vec::new(),
                rewards: Vec::new(),
                unknown_text: None,
            }],
        };
        let mut combat = CombatTargetModel::default();
        combat.apply(mir2_client_bevy::quest_model::CombatTargetUpdate {
            object_id: 42,
            name: "Scarecrow".to_owned(),
            hp: 5,
            max_hp: 10,
            is_player: false,
        });

        assert!(capture_target_matches(
            NativeCaptureTarget::QuestAccepted,
            &shell,
            Some(&tracker),
            Some(&combat),
            Some(2)
        ));
        assert!(capture_target_matches(
            NativeCaptureTarget::Combat,
            &shell,
            Some(&tracker),
            Some(&combat),
            Some(2)
        ));
        tracker.active_quests[0].status = QuestStatus::Completed;
        assert!(capture_target_matches(
            NativeCaptureTarget::QuestComplete,
            &shell,
            Some(&tracker),
            Some(&combat),
            Some(2)
        ));
        assert!(!capture_target_matches(
            NativeCaptureTarget::QuestAccepted,
            &shell,
            Some(&tracker),
            Some(&combat),
            Some(2)
        ));
    }

    #[test]
    fn sidecar_freezes_observed_state_and_marks_missing_provenance_non_accepting() {
        let mut shell = NativeShellModel::default();
        shell.screen = NativeShellScreen::InGame;
        let entities = EntityModelSet {
            entities: vec![mir2_client_bevy::entities::EntityModel {
                object_id: "1000".to_owned(),
                kind: EntityKind::SelfPlayer,
                name: "Adventurer".to_owned(),
                x: 287,
                y: 618,
                level: None,
                direction: None,
            }],
        };
        let ui = UiReadModel {
            player: mir2_client_bevy::read_model::PlayerStats {
                map_name: Some("BichonProvince".to_owned()),
                ..Default::default()
            },
        };
        let mut player_ui = NativePlayerUiState::default();
        player_ui.core.login_account = "must-not-leak".to_owned();
        player_ui.core.login_password = "must-not-leak".to_owned();
        let request = capture_request(
            PathBuf::from("captures/native-in-game-1.png"),
            "in-game",
            Some(&shell),
            Some(&ui),
            Some(&entities),
            Some(&player_ui),
            Some(1.25),
        );
        let sidecar: serde_json::Value = serde_json::from_slice(
            &serialize_capture_sidecar(&request, 1024, 768, "a".repeat(64)).expect("sidecar"),
        )
        .expect("sidecar json");

        assert_eq!(sidecar["schemaVersion"], CAPTURE_DRAFT_SCHEMA_VERSION);
        assert_eq!(sidecar["imagePath"], "native-in-game-1.png");
        assert_eq!(sidecar["world"]["map"], "BichonProvince");
        assert_eq!(sidecar["world"]["x"], 287);
        assert_eq!(sidecar["world"]["y"], 618);
        assert!(sidecar["world"]["light"].is_null());
        assert_eq!(sidecar["dpiScale"], 1.25);
        assert_eq!(sidecar["build"]["sourceRevision"], serde_json::Value::Null);
        assert_eq!(sidecar["acceptance"]["eligible"], false);
        let ui_state = sidecar["uiState"].as_str().expect("safe UI state");
        assert!(ui_state.contains("panel=None"));
        assert!(!ui_state.contains("must-not-leak"));
    }

    #[test]
    fn capture_run_id_requires_the_pair_validator_safe_character_set() {
        assert_eq!(
            sanitize_capture_run_id("run-24.08_A"),
            Some("run-24.08_A".to_owned())
        );
        assert_eq!(
            sanitize_capture_run_id(" invalid"),
            Some("invalid".to_owned())
        );
        assert_eq!(sanitize_capture_run_id("run id"), None);
        assert_eq!(sanitize_capture_run_id("../run"), None);
    }

    #[test]
    fn v1_sidecar_is_closed_only_when_every_required_field_is_present() {
        let mut request = capture_request(
            PathBuf::from("captures/native-login-1.png"),
            "login",
            None,
            None,
            None,
            Some(&NativePlayerUiState::default()),
            Some(1.0),
        );
        request.run_id = Some("pair-001".to_owned());
        request.build = NativeCaptureBuild {
            source_revision: Some("test-source".to_owned()),
            executable_sha256: Some("a".repeat(64)),
            asset_manifest_sha256: Some("b".repeat(64)),
        };
        let sidecar: serde_json::Value = serde_json::from_slice(
            &serialize_capture_sidecar(&request, 1024, 768, "c".repeat(64)).expect("sidecar"),
        )
        .expect("v1 json");

        assert_eq!(sidecar["schemaVersion"], CAPTURE_SCHEMA_VERSION);
        assert!(sidecar.get("acceptance").is_none());
        assert_eq!(sidecar["world"], serde_json::Value::Null);
    }

    #[test]
    fn sha256_and_utc_timestamp_match_known_values() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(format_rfc3339_utc(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(
            format_rfc3339_utc(1_724_428_800_123),
            "2024-08-23T16:00:00.123Z"
        );
    }

    #[test]
    fn atomic_writer_never_replaces_an_existing_file() {
        let path = env::temp_dir().join(format!(
            "mir2-native-capture-atomic-{}-{}.json",
            std::process::id(),
            CAPTURE_TEMP_INDEX.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_file(&path);
        write_atomic_bytes(&path, b"first").expect("first atomic write");
        assert_eq!(fs::read(&path).expect("first bytes"), b"first");
        assert!(write_atomic_bytes(&path, b"second").is_err());
        assert_eq!(fs::read(&path).expect("preserved bytes"), b"first");
        let _ = fs::remove_file(path);
    }
}
