//! Native adapter for renderer-neutral chat settings effects.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use bevy::prelude::{ResMut, Resource};
use mir2_ui_core::effect::UiEffect;
use mir2_ui_core::state::UiChatSettings;
use serde::{Deserialize, Serialize};

use crate::crystal_ui::chat::CrystalChatState;
use crate::crystal_ui::overlays::{NativePlayerUiState, UiEffectQueue};

pub const MAX_CHAT_SETTINGS_EFFECTS_PER_TICK: usize = 8;
const CHAT_SETTINGS_SCHEMA_VERSION: u8 = 1;
const CHAT_SETTINGS_FILE: &str = "chat-settings.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatSettingsLoadSource {
    Defaults,
    Primary,
    Backup,
}

#[derive(Debug, Clone, PartialEq, Eq, Resource)]
pub struct ChatSettingsRuntime {
    pub config_path: PathBuf,
    pub loaded_from: ChatSettingsLoadSource,
    pub persisted_settings: Option<UiChatSettings>,
    pub last_error: Option<String>,
}

impl Default for ChatSettingsRuntime {
    fn default() -> Self {
        Self::with_config_path(chat_settings_config_path())
    }
}

impl ChatSettingsRuntime {
    pub fn with_config_path(config_path: PathBuf) -> Self {
        Self {
            config_path,
            loaded_from: ChatSettingsLoadSource::Defaults,
            persisted_settings: None,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PersistedChatSettings {
    version: u8,
    filter_normal: bool,
    filter_whisper: bool,
    filter_shout: bool,
    filter_system: bool,
    filter_lover: bool,
    filter_mentor: bool,
    filter_group: bool,
    filter_guild: bool,
    filter_trade: bool,
    transparent: bool,
}

impl From<UiChatSettings> for PersistedChatSettings {
    fn from(settings: UiChatSettings) -> Self {
        Self {
            version: CHAT_SETTINGS_SCHEMA_VERSION,
            filter_normal: settings.filter_normal,
            filter_whisper: settings.filter_whisper,
            filter_shout: settings.filter_shout,
            filter_system: settings.filter_system,
            filter_lover: settings.filter_lover,
            filter_mentor: settings.filter_mentor,
            filter_group: settings.filter_group,
            filter_guild: settings.filter_guild,
            filter_trade: settings.filter_trade,
            transparent: settings.transparent,
        }
    }
}

impl TryFrom<PersistedChatSettings> for UiChatSettings {
    type Error = ();

    fn try_from(value: PersistedChatSettings) -> Result<Self, Self::Error> {
        if value.version != CHAT_SETTINGS_SCHEMA_VERSION {
            return Err(());
        }
        Ok(Self {
            filter_normal: value.filter_normal,
            filter_whisper: value.filter_whisper,
            filter_shout: value.filter_shout,
            filter_system: value.filter_system,
            filter_lover: value.filter_lover,
            filter_mentor: value.filter_mentor,
            filter_group: value.filter_group,
            filter_guild: value.filter_guild,
            filter_trade: value.filter_trade,
            transparent: value.transparent,
        })
    }
}

pub fn chat_settings_config_path() -> PathBuf {
    crate::options_effects::options_config_path().with_file_name(CHAT_SETTINGS_FILE)
}

fn backup_path(path: &Path) -> PathBuf {
    path.with_extension("json.bak")
}

fn temporary_path(path: &Path) -> PathBuf {
    path.with_extension("json.tmp")
}

fn decode(bytes: &[u8]) -> Option<UiChatSettings> {
    serde_json::from_slice::<PersistedChatSettings>(bytes)
        .ok()
        .and_then(|value| UiChatSettings::try_from(value).ok())
}

pub fn load_chat_settings_from_path(path: &Path) -> (UiChatSettings, ChatSettingsLoadSource) {
    if let Ok(bytes) = fs::read(path) {
        if let Some(settings) = decode(&bytes) {
            return (settings, ChatSettingsLoadSource::Primary);
        }
    }
    if let Ok(bytes) = fs::read(backup_path(path)) {
        if let Some(settings) = decode(&bytes) {
            return (settings, ChatSettingsLoadSource::Backup);
        }
    }
    (UiChatSettings::default(), ChatSettingsLoadSource::Defaults)
}

pub fn persist_chat_settings_to_path(path: &Path, settings: UiChatSettings) -> io::Result<()> {
    let payload = serde_json::to_vec_pretty(&PersistedChatSettings::from(settings))
        .map_err(io::Error::other)?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
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
    if let Err(error) = fs::rename(&temporary, path) {
        if had_primary && !path.exists() {
            let _ = fs::rename(&backup, path);
        }
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    if let Ok(directory) = File::open(parent) {
        let _ = directory.sync_all();
    }
    Ok(())
}

pub fn load_persisted_chat_settings(
    mut player_ui: ResMut<NativePlayerUiState>,
    mut renderer: ResMut<CrystalChatState>,
    mut runtime: ResMut<ChatSettingsRuntime>,
) {
    let (settings, source) = load_chat_settings_from_path(&runtime.config_path);
    player_ui.core.chat_settings = settings;
    player_ui.core.chat_settings_draft = None;
    renderer.applied_settings = settings;
    runtime.loaded_from = source;
    runtime.persisted_settings = match source {
        ChatSettingsLoadSource::Defaults => None,
        ChatSettingsLoadSource::Primary | ChatSettingsLoadSource::Backup => Some(settings),
    };
    runtime.last_error = None;
}

pub fn consume_chat_settings_effects(
    mut queue: ResMut<UiEffectQueue>,
    mut renderer: ResMut<CrystalChatState>,
    mut runtime: ResMut<ChatSettingsRuntime>,
) {
    for effect in queue.drain_chat_settings_bounded(MAX_CHAT_SETTINGS_EFFECTS_PER_TICK) {
        match effect {
            UiEffect::ApplyChatSettings { settings } => {
                renderer.applied_settings = settings;
            }
            UiEffect::PersistChatSettings { settings } => {
                if runtime.persisted_settings == Some(settings) && runtime.config_path.is_file() {
                    continue;
                }
                match persist_chat_settings_to_path(&runtime.config_path, settings) {
                    Ok(()) => {
                        runtime.persisted_settings = Some(settings);
                        runtime.last_error = None;
                    }
                    Err(error) => runtime.last_error = Some(error.to_string()),
                }
            }
            _ => unreachable!("chat-settings drain returned a non-chat effect"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::App;
    use mir2_ui_core::effect::{GatewayCommand, UiEffect};
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TestPath(PathBuf);

    impl TestPath {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!(
                "mir2-chat-settings-{}-{}.json",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::Relaxed)
            )))
        }
    }

    impl Drop for TestPath {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
            let _ = fs::remove_file(backup_path(&self.0));
            let _ = fs::remove_file(temporary_path(&self.0));
        }
    }

    fn sample() -> UiChatSettings {
        UiChatSettings {
            filter_guild: true,
            transparent: true,
            ..Default::default()
        }
    }

    fn app(path: &Path) -> App {
        let mut app = App::new();
        app.init_resource::<UiEffectQueue>()
            .init_resource::<CrystalChatState>()
            .insert_resource(ChatSettingsRuntime::with_config_path(path.to_owned()))
            .add_systems(bevy::app::Update, consume_chat_settings_effects);
        app
    }

    #[test]
    fn apply_and_persist_share_one_payload_without_swallowing_gateway_effects() {
        let path = TestPath::new();
        let settings = sample();
        let mut app = app(&path.0);
        {
            let mut queue = app.world_mut().resource_mut::<UiEffectQueue>();
            queue.push(UiEffect::GatewayCommand(GatewayCommand::Logout));
            queue.push(UiEffect::ApplyChatSettings { settings });
            queue.push(UiEffect::PersistChatSettings { settings });
        }
        app.update();
        assert_eq!(
            app.world().resource::<CrystalChatState>().applied_settings,
            settings
        );
        assert_eq!(load_chat_settings_from_path(&path.0).0, settings);
        assert!(matches!(
            app.world_mut()
                .resource_mut::<UiEffectQueue>()
                .drain()
                .as_slice(),
            [UiEffect::GatewayCommand(GatewayCommand::Logout)]
        ));
    }

    #[test]
    fn corrupt_primary_uses_backup_then_defaults() {
        let path = TestPath::new();
        persist_chat_settings_to_path(&path.0, UiChatSettings::default()).unwrap();
        persist_chat_settings_to_path(&path.0, sample()).unwrap();
        fs::write(&path.0, b"bad-json").unwrap();
        assert_eq!(
            load_chat_settings_from_path(&path.0),
            (UiChatSettings::default(), ChatSettingsLoadSource::Backup)
        );
        fs::write(backup_path(&path.0), b"bad-backup").unwrap();
        assert_eq!(
            load_chat_settings_from_path(&path.0),
            (UiChatSettings::default(), ChatSettingsLoadSource::Defaults)
        );
    }

    #[test]
    fn persisted_file_is_an_explicit_non_secret_allow_list() {
        let path = TestPath::new();
        persist_chat_settings_to_path(&path.0, sample()).unwrap();
        let text = fs::read_to_string(&path.0).unwrap().to_ascii_lowercase();
        for secret in ["account", "password", "token", "secret", "passkey"] {
            assert!(!text.contains(secret), "secret field leaked: {secret}");
        }
    }
}
