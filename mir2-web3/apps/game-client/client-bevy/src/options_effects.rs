//! Native Bevy consumers for the platform-agnostic Options effects.
//!
//! `ui-core` deliberately emits typed effects without knowing about Bevy's
//! window or audio APIs. This module is the native adapter boundary: it drains
//! only a bounded number of Options effects per frame, applies a real Bevy
//! window mode when a primary window exists, and persists an explicit
//! non-secret allow-list of settings. The native audio adapter owns the Bevy
//! playback entities; this module remains responsible only for consuming the
//! typed settings effect and persisting the allow-list.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use bevy::prelude::{Query, ResMut, Resource, Window, With};
use bevy::window::{MonitorSelection, PrimaryWindow, WindowMode};
use serde::{Deserialize, Serialize};

use crate::crystal_ui::overlays::{NativePlayerUiState, UiEffectQueue};
use mir2_ui_core::effect::UiEffect;
use mir2_ui_core::state::{UiOptions, UiPlatformSettings, UiWindowMode};

pub const MAX_OPTIONS_EFFECTS_PER_TICK: usize = 8;

/// Version 2 adds the seven local switches owned by Crystal's OptionDialog.
/// Version 1 remains readable so an existing local audio/window configuration
/// is upgraded with Crystal's defaults rather than discarded.
const OPTIONS_SCHEMA_VERSION: u8 = 2;
const LEGACY_OPTIONS_SCHEMA_VERSION: u8 = 1;
const OPTIONS_DIRECTORY: &str = "mir2-web3";
const OPTIONS_FILE: &str = "options.json";

/// The audio settings accepted by the native adapter.
///
/// `audible_backend` is set by the native audio adapter after it finds at least
/// one real source. It stays false when the source bundle is absent; a missing
/// sound device or file never makes the UI path fail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedAudioSettings {
    pub music_enabled: bool,
    pub music_volume: u8,
    pub sound_enabled: bool,
    pub sound_volume: u8,
    pub audible_backend: bool,
}

impl AppliedAudioSettings {
    fn from_values(
        music_enabled: bool,
        music_volume: u8,
        sound_enabled: bool,
        sound_volume: u8,
    ) -> Self {
        Self {
            music_enabled,
            music_volume: UiOptions::clamp_volume(music_volume),
            sound_enabled,
            sound_volume: UiOptions::clamp_volume(sound_volume),
            audible_backend: false,
        }
    }

    fn from_options(options: &UiOptions) -> Self {
        Self::from_values(
            options.music_enabled,
            options.music_volume,
            options.sound_enabled,
            options.sound_volume,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionsLoadSource {
    Defaults,
    Primary,
    Backup,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedOptions {
    pub options: UiOptions,
    pub source: OptionsLoadSource,
}

/// Runtime evidence for the native Options adapter.
#[derive(Debug, Clone, PartialEq, Eq, Resource)]
pub struct OptionsRuntime {
    pub audio: AppliedAudioSettings,
    /// Monotonic revision used by the audio adapter to turn an Apply action
    /// into one repeatable sound trigger without coupling sound to UI code.
    pub audio_revision: u64,
    pub window_mode: UiWindowMode,
    pub window_available: bool,
    pub config_path: PathBuf,
    pub loaded_from: OptionsLoadSource,
    pub persisted_options: Option<UiOptions>,
    pub last_error: Option<String>,
}

impl Default for OptionsRuntime {
    fn default() -> Self {
        Self::with_config_path(options_config_path())
    }
}

impl OptionsRuntime {
    pub fn with_config_path(config_path: PathBuf) -> Self {
        let options = UiOptions::default();
        Self {
            audio: AppliedAudioSettings::from_options(&options),
            audio_revision: 0,
            window_mode: options.window_mode,
            window_available: false,
            config_path,
            loaded_from: OptionsLoadSource::Defaults,
            persisted_options: None,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct PersistedOptions {
    version: u8,
    #[serde(default)]
    skill_mode: bool,
    #[serde(default = "default_true")]
    skill_bar: bool,
    #[serde(default = "default_true")]
    effect: bool,
    #[serde(default = "default_true")]
    drop_view: bool,
    #[serde(default = "default_true")]
    name_view: bool,
    #[serde(default = "default_true")]
    hp_view: bool,
    #[serde(default)]
    new_move: bool,
    music_enabled: bool,
    music_volume: u8,
    sound_enabled: bool,
    sound_volume: u8,
    window_mode: UiWindowMode,
}

fn default_true() -> bool {
    true
}

impl From<&UiOptions> for PersistedOptions {
    fn from(options: &UiOptions) -> Self {
        Self {
            version: OPTIONS_SCHEMA_VERSION,
            skill_mode: options.skill_mode,
            skill_bar: options.skill_bar,
            effect: options.effect,
            drop_view: options.drop_view,
            name_view: options.name_view,
            hp_view: options.hp_view,
            new_move: options.new_move,
            music_enabled: options.music_enabled,
            music_volume: options.music_volume,
            sound_enabled: options.sound_enabled,
            sound_volume: options.sound_volume,
            window_mode: options.window_mode,
        }
    }
}

impl TryFrom<PersistedOptions> for UiOptions {
    type Error = ();

    fn try_from(value: PersistedOptions) -> Result<Self, Self::Error> {
        if !matches!(
            value.version,
            LEGACY_OPTIONS_SCHEMA_VERSION | OPTIONS_SCHEMA_VERSION
        ) || value.music_volume > UiOptions::MAX_VOLUME
            || value.sound_volume > UiOptions::MAX_VOLUME
        {
            return Err(());
        }
        Ok(UiOptions {
            skill_mode: value.skill_mode,
            skill_bar: value.skill_bar,
            effect: value.effect,
            drop_view: value.drop_view,
            name_view: value.name_view,
            hp_view: value.hp_view,
            new_move: value.new_move,
            music_enabled: value.music_enabled,
            music_volume: value.music_volume,
            sound_enabled: value.sound_enabled,
            sound_volume: value.sound_volume,
            window_mode: value.window_mode,
        })
    }
}

fn sanitize_options(mut options: UiOptions) -> UiOptions {
    options.music_volume = UiOptions::clamp_volume(options.music_volume);
    options.sound_volume = UiOptions::clamp_volume(options.sound_volume);
    options
}

/// Return the deterministic per-user native config path.
pub fn options_config_path() -> PathBuf {
    #[cfg(windows)]
    let base = std::env::var_os("APPDATA")
        .or_else(|| std::env::var_os("LOCALAPPDATA"))
        .map(PathBuf::from);

    #[cfg(not(windows))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")));

    base.unwrap_or_else(|| {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".config")
    })
    .join(OPTIONS_DIRECTORY)
    .join(OPTIONS_FILE)
}

fn backup_path(path: &Path) -> PathBuf {
    path.with_extension("json.bak")
}

fn temporary_path(path: &Path) -> PathBuf {
    path.with_extension("json.tmp")
}

fn decode_options(bytes: &[u8]) -> Option<UiOptions> {
    serde_json::from_slice::<PersistedOptions>(bytes)
        .ok()
        .and_then(|stored| UiOptions::try_from(stored).ok())
}

/// Load the primary file, then the recoverable backup, then safe defaults.
pub fn load_options_from_path(path: &Path) -> LoadedOptions {
    if let Ok(bytes) = fs::read(path) {
        if let Some(options) = decode_options(&bytes) {
            return LoadedOptions {
                options,
                source: OptionsLoadSource::Primary,
            };
        }
    }

    let backup = backup_path(path);
    if let Ok(bytes) = fs::read(backup) {
        if let Some(options) = decode_options(&bytes) {
            return LoadedOptions {
                options,
                source: OptionsLoadSource::Backup,
            };
        }
    }

    LoadedOptions {
        options: UiOptions::default(),
        source: OptionsLoadSource::Defaults,
    }
}

fn sync_parent_directory(parent: &Path) {
    // Directory fsync is useful on Unix after a rename. It is not available on
    // every Windows filesystem, so failure is deliberately non-fatal: the
    // file itself was flushed and the backup remains recoverable.
    if let Ok(directory) = File::open(parent) {
        let _ = directory.sync_all();
    }
}

/// Persist only the explicit Options allow-list using a recoverable replace.
pub fn persist_options_to_path(path: &Path, options: &UiOptions) -> io::Result<()> {
    let options = sanitize_options(options.clone());
    let payload =
        serde_json::to_vec_pretty(&PersistedOptions::from(&options)).map_err(io::Error::other)?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    let parent = parent.unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;

    let temporary = temporary_path(path);
    let backup = backup_path(path);
    {
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temporary)?;
        file.write_all(&payload)?;
        file.flush()?;
        file.sync_all()?;
    }

    let had_primary = path.is_file();
    if had_primary {
        if backup.is_file() {
            fs::remove_file(&backup)?;
        }
        fs::rename(path, &backup)?;
    }

    match fs::rename(&temporary, path) {
        Ok(()) => {
            sync_parent_directory(parent);
            Ok(())
        }
        Err(error) => {
            // Restore the previous primary if the replacement failed. If the
            // restore itself fails, the .bak remains available for next start.
            if had_primary && !path.exists() {
                let _ = fs::rename(&backup, path);
            }
            let _ = fs::remove_file(&temporary);
            Err(error)
        }
    }
}

fn apply_bevy_window_mode(
    windows: &mut Query<&mut Window, With<PrimaryWindow>>,
    mode: UiWindowMode,
) -> bool {
    let Some(mut window) = windows.iter_mut().next() else {
        return false;
    };
    let desired = match mode {
        UiWindowMode::Windowed => WindowMode::Windowed,
        // Borderless fullscreen avoids guessing a resolution/video mode and
        // lets Bevy/winit select the current monitor safely.
        UiWindowMode::Fullscreen => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
    };
    if window.mode != desired {
        window.mode = desired;
    }
    true
}

/// Startup adapter: load local options and apply the initial window state.
pub fn load_persisted_options(
    mut state: ResMut<NativePlayerUiState>,
    mut runtime: ResMut<OptionsRuntime>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let loaded = load_options_from_path(&runtime.config_path);
    state.core.options = loaded.options.clone();
    state.core.options_draft = None;
    state.core.platform_settings = UiPlatformSettings {
        window_mode: loaded.options.window_mode,
    };
    state.core.platform_settings_draft = None;
    runtime.audio = AppliedAudioSettings::from_options(&loaded.options);
    runtime.window_mode = loaded.options.window_mode;
    runtime.window_available = apply_bevy_window_mode(&mut windows, loaded.options.window_mode);
    runtime.loaded_from = loaded.source;
    runtime.persisted_options = match loaded.source {
        OptionsLoadSource::Defaults => None,
        OptionsLoadSource::Primary | OptionsLoadSource::Backup => Some(loaded.options),
    };
    runtime.last_error = None;
}

fn apply_audio_settings(
    runtime: &mut OptionsRuntime,
    music_enabled: bool,
    music_volume: u8,
    sound_enabled: bool,
    sound_volume: u8,
) {
    let audible_backend = runtime.audio.audible_backend;
    let applied =
        AppliedAudioSettings::from_values(music_enabled, music_volume, sound_enabled, sound_volume);
    runtime.audio = AppliedAudioSettings {
        audible_backend,
        ..applied
    };
    runtime.audio_revision = runtime.audio_revision.wrapping_add(1);
}

fn apply_window_mode(
    runtime: &mut OptionsRuntime,
    windows: &mut Query<&mut Window, With<PrimaryWindow>>,
    mode: UiWindowMode,
) {
    runtime.window_mode = mode;
    runtime.window_available = apply_bevy_window_mode(windows, mode);
}

fn persist_options(runtime: &mut OptionsRuntime, options: UiOptions) {
    let options = sanitize_options(options);
    if runtime.persisted_options.as_ref() == Some(&options) && runtime.config_path.is_file() {
        return;
    }
    match persist_options_to_path(&runtime.config_path, &options) {
        Ok(()) => {
            runtime.persisted_options = Some(options);
            runtime.last_error = None;
        }
        Err(error) => {
            runtime.last_error = Some(error.to_string());
        }
    }
}

/// Drain and apply at most [`MAX_OPTIONS_EFFECTS_PER_TICK`] Options effects.
/// Other UI effects remain in the shared queue for their owning host adapter.
pub fn consume_options_effects(
    mut queue: ResMut<UiEffectQueue>,
    mut runtime: ResMut<OptionsRuntime>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    for effect in queue.drain_options_bounded(MAX_OPTIONS_EFFECTS_PER_TICK) {
        match effect {
            UiEffect::ApplyAudioSettings {
                music_enabled,
                music_volume,
                sound_enabled,
                sound_volume,
            } => apply_audio_settings(
                &mut runtime,
                music_enabled,
                music_volume,
                sound_enabled,
                sound_volume,
            ),
            UiEffect::ApplyWindowMode { mode } => {
                apply_window_mode(&mut runtime, &mut windows, mode)
            }
            UiEffect::PersistOptions { options } => persist_options(&mut runtime, options),
            _ => unreachable!("UiEffectQueue::drain_options_bounded filtered this effect"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::{App, Window};
    use bevy::window::PrimaryWindow;
    use mir2_ui_core::effect::{GatewayCommand, UiEffect};
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TestPath {
        root: PathBuf,
        file: PathBuf,
    }

    impl TestPath {
        fn new(label: &str) -> Self {
            let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "mir2-options-effects-{label}-{}-{id}",
                std::process::id()
            ));
            Self {
                file: root.join("config").join(OPTIONS_FILE),
                root,
            }
        }
    }

    impl Drop for TestPath {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn sample_options() -> UiOptions {
        UiOptions {
            skill_mode: true,
            skill_bar: false,
            effect: false,
            drop_view: false,
            name_view: false,
            hp_view: false,
            new_move: true,
            music_enabled: false,
            music_volume: 35,
            sound_enabled: true,
            sound_volume: 65,
            window_mode: UiWindowMode::Fullscreen,
        }
    }

    fn app_with_options(path: &Path) -> App {
        let mut app = App::new();
        app.init_resource::<UiEffectQueue>()
            .insert_resource(OptionsRuntime::with_config_path(path.to_owned()))
            .add_systems(bevy::app::Update, consume_options_effects);
        app.world_mut().spawn((Window::default(), PrimaryWindow));
        app
    }

    #[test]
    fn apply_consumes_audio_window_and_persistence_effects() {
        let test_path = TestPath::new("apply");
        let options = sample_options();
        let mut app = app_with_options(&test_path.file);
        {
            let mut queue = app.world_mut().resource_mut::<UiEffectQueue>();
            queue.push(UiEffect::ApplyAudioSettings {
                music_enabled: options.music_enabled,
                music_volume: options.music_volume,
                sound_enabled: options.sound_enabled,
                sound_volume: options.sound_volume,
            });
            queue.push(UiEffect::ApplyWindowMode {
                mode: options.window_mode,
            });
            queue.push(UiEffect::PersistOptions {
                options: options.clone(),
            });
        }
        app.update();

        let runtime = app.world().resource::<OptionsRuntime>();
        assert_eq!(runtime.audio.music_volume, 35);
        assert!(!runtime.audio.music_enabled);
        assert_eq!(runtime.audio.sound_volume, 65);
        assert!(!runtime.audio.audible_backend);
        assert_eq!(runtime.window_mode, UiWindowMode::Fullscreen);
        assert!(runtime.window_available);
        assert_eq!(runtime.persisted_options.as_ref(), Some(&options));
        assert_eq!(load_options_from_path(&test_path.file).options, options);

        let mode = app
            .world_mut()
            .query::<&Window>()
            .single(app.world())
            .expect("the primary window")
            .mode
            .clone();
        assert!(matches!(mode, WindowMode::BorderlessFullscreen(_)));
    }

    #[test]
    fn corrupt_primary_falls_back_to_backup_then_defaults() {
        let test_path = TestPath::new("recovery");
        let first = UiOptions::default();
        let second = sample_options();
        persist_options_to_path(&test_path.file, &first).expect("first write");
        persist_options_to_path(&test_path.file, &second).expect("second write");
        fs::write(&test_path.file, b"not-json").expect("corrupt primary");

        let recovered = load_options_from_path(&test_path.file);
        assert_eq!(recovered.source, OptionsLoadSource::Backup);
        assert_eq!(recovered.options, first);

        fs::write(backup_path(&test_path.file), b"also-not-json").expect("corrupt backup");
        let fallback = load_options_from_path(&test_path.file);
        assert_eq!(fallback.source, OptionsLoadSource::Defaults);
        assert_eq!(fallback.options, UiOptions::default());
    }

    #[test]
    fn repeated_identical_effects_are_idempotent_and_non_option_effects_survive() {
        let test_path = TestPath::new("idempotent");
        let options = sample_options();
        let mut app = app_with_options(&test_path.file);
        {
            let mut queue = app.world_mut().resource_mut::<UiEffectQueue>();
            for _ in 0..2 {
                queue.push(UiEffect::ApplyAudioSettings {
                    music_enabled: options.music_enabled,
                    music_volume: options.music_volume,
                    sound_enabled: options.sound_enabled,
                    sound_volume: options.sound_volume,
                });
                queue.push(UiEffect::ApplyWindowMode {
                    mode: options.window_mode,
                });
                queue.push(UiEffect::PersistOptions {
                    options: options.clone(),
                });
            }
            queue.push(UiEffect::GatewayCommand(GatewayCommand::Logout));
        }
        app.update();
        let first_runtime = app.world().resource::<OptionsRuntime>().clone();
        let first_bytes = fs::read(&test_path.file).expect("persisted options");
        app.update();
        let second_runtime = app.world().resource::<OptionsRuntime>().clone();
        let second_bytes = fs::read(&test_path.file).expect("persisted options");
        assert_eq!(first_runtime, second_runtime);
        assert_eq!(first_bytes, second_bytes);
        assert_eq!(app.world().resource::<UiEffectQueue>().len(), 1);
        assert!(matches!(
            app.world_mut()
                .resource_mut::<UiEffectQueue>()
                .drain()
                .as_slice(),
            [UiEffect::GatewayCommand(GatewayCommand::Logout)]
        ));
    }

    #[test]
    fn options_drain_is_bounded_per_update() {
        let test_path = TestPath::new("bounded");
        let mut app = app_with_options(&test_path.file);
        {
            let mut queue = app.world_mut().resource_mut::<UiEffectQueue>();
            for _ in 0..MAX_OPTIONS_EFFECTS_PER_TICK + 2 {
                queue.push(UiEffect::ApplyWindowMode {
                    mode: UiWindowMode::Fullscreen,
                });
            }
        }
        app.update();
        assert_eq!(
            app.world().resource::<UiEffectQueue>().len(),
            2,
            "one frame must not drain the entire producer queue"
        );
        app.update();
        assert_eq!(app.world().resource::<UiEffectQueue>().len(), 0);
    }

    #[test]
    fn startup_loads_persisted_options_and_applies_window_mode() {
        let test_path = TestPath::new("startup");
        let options = sample_options();
        persist_options_to_path(&test_path.file, &options).expect("persisted options");
        let mut app = App::new();
        app.init_resource::<NativePlayerUiState>()
            .insert_resource(OptionsRuntime::with_config_path(test_path.file.clone()))
            .add_systems(bevy::app::Startup, load_persisted_options);
        app.world_mut().spawn((Window::default(), PrimaryWindow));
        app.update();
        assert_eq!(
            app.world().resource::<NativePlayerUiState>().core.options,
            options
        );
        let runtime = app.world().resource::<OptionsRuntime>();
        assert_eq!(runtime.loaded_from, OptionsLoadSource::Primary);
        assert!(runtime.window_available);
        let mode = app
            .world_mut()
            .query::<&Window>()
            .single(app.world())
            .expect("primary window")
            .mode
            .clone();
        assert!(matches!(mode, WindowMode::BorderlessFullscreen(_)));
    }

    #[test]
    fn persisted_payload_contains_only_non_secret_allow_list() {
        let test_path = TestPath::new("no-secrets");
        persist_options_to_path(&test_path.file, &sample_options()).expect("persisted options");
        let bytes = fs::read(&test_path.file).expect("config file");
        let text = String::from_utf8(bytes.clone()).expect("utf8 json");
        let value: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        let keys = value
            .as_object()
            .expect("object")
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(
            keys,
            vec![
                "drop_view",
                "effect",
                "hp_view",
                "music_enabled",
                "music_volume",
                "name_view",
                "new_move",
                "skill_bar",
                "skill_mode",
                "sound_enabled",
                "sound_volume",
                "version",
                "window_mode"
            ]
        );
        let lower = text.to_ascii_lowercase();
        for secret_name in ["account", "password", "token", "secret", "passkey"] {
            assert!(
                !lower.contains(secret_name),
                "secret field leaked: {secret_name}"
            );
        }
    }

    #[test]
    fn version_one_options_upgrade_with_crystal_defaults() {
        let legacy = br#"{
            "version": 1,
            "music_enabled": false,
            "music_volume": 25,
            "sound_enabled": true,
            "sound_volume": 75,
            "window_mode": "Fullscreen"
        }"#;
        let options = decode_options(legacy).expect("legacy options should load");
        assert_eq!(options.crystal(), UiOptions::default().crystal());
        assert!(!options.music_enabled);
        assert_eq!(options.music_volume, 25);
        assert_eq!(options.window_mode, UiWindowMode::Fullscreen);
    }

    #[test]
    fn all_crystal_option_fields_round_trip_through_persistence() {
        let test_path = TestPath::new("crystal-fields");
        let expected = sample_options();
        persist_options_to_path(&test_path.file, &expected).expect("persist options");
        let loaded = load_options_from_path(&test_path.file);
        assert_eq!(loaded.source, OptionsLoadSource::Primary);
        assert_eq!(loaded.options, expected);
        let payload = fs::read_to_string(&test_path.file).expect("options json");
        for field in [
            "skill_mode",
            "skill_bar",
            "effect",
            "drop_view",
            "name_view",
            "hp_view",
            "new_move",
        ] {
            assert!(payload.contains(field), "missing persisted field: {field}");
        }
    }
}
