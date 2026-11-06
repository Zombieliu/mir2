//! Native persistence adapter for Crystal's F1-F8 skill bindings.
//!
//! The adapter deliberately owns no panel or input code.  `SkillBindingUi`
//! already provides the bounded, sanitizing serde contract, so this module is
//! responsible only for choosing the per-user path, recovering a usable file,
//! and reporting whether a persist actually succeeded.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use bevy::app::{App, Plugin, Startup};
use bevy::prelude::{ResMut, Resource};

use crate::skill_binding_ui::SkillBindingUi;

/// The only file written by this adapter. It lives beside `options.json`.
pub const SKILL_BINDINGS_FILE: &str = "skill-bindings.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillBindingLoadSource {
    Defaults,
    Primary,
    Backup,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedSkillBindings {
    pub bindings: SkillBindingUi,
    pub source: SkillBindingLoadSource,
}

/// The last persistence transition is intentionally observable by the host.
/// In particular, `Failed` is distinct from `Succeeded` and does not update
/// `persisted_bindings`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillBindingPersistStatus {
    NeverAttempted,
    Succeeded,
    SkippedUnchanged,
    Failed(String),
}

/// Bevy resource shared by the eventual thin UI adapter.
#[derive(Debug, Clone, PartialEq, Eq, Resource)]
pub struct SkillBindingPersistenceRuntime {
    pub config_path: PathBuf,
    pub loaded_from: SkillBindingLoadSource,
    /// The last binding set confirmed on disk. `None` means no valid file has
    /// been loaded or written yet; it does not mean the current UI is empty.
    pub persisted_bindings: Option<SkillBindingUi>,
    pub dirty: bool,
    pub last_status: SkillBindingPersistStatus,
    pub last_error: Option<String>,
}

impl Default for SkillBindingPersistenceRuntime {
    fn default() -> Self {
        Self::with_config_path(skill_bindings_config_path())
    }
}

impl SkillBindingPersistenceRuntime {
    pub fn with_config_path(config_path: PathBuf) -> Self {
        Self {
            config_path,
            loaded_from: SkillBindingLoadSource::Defaults,
            persisted_bindings: None,
            dirty: false,
            last_status: SkillBindingPersistStatus::NeverAttempted,
            last_error: None,
        }
    }

    /// Mark the current UI state as needing a persistence attempt.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }
}

/// A convenient native host plugin. It only installs the Resource values and
/// the Startup load; a later overlay adapter can call
/// [`persist_skill_bindings_if_changed`] after a successful rebind.
#[derive(Debug, Default)]
pub struct SkillBindingPersistencePlugin;

impl Plugin for SkillBindingPersistencePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SkillBindingUi>()
            .init_resource::<SkillBindingPersistenceRuntime>()
            .add_systems(Startup, load_persisted_skill_bindings);
    }
}

/// Return the deterministic per-user path in the same directory as Options.
pub fn skill_bindings_config_path() -> PathBuf {
    crate::options_effects::options_config_path().with_file_name(SKILL_BINDINGS_FILE)
}

fn backup_path(path: &Path) -> PathBuf {
    path.with_extension("json.bak")
}

fn temporary_path(path: &Path) -> PathBuf {
    path.with_extension("json.tmp")
}

fn decode(bytes: &[u8]) -> Option<SkillBindingUi> {
    serde_json::from_slice(bytes).ok()
}

/// Load primary, then backup, then a safe empty binding set.
pub fn load_skill_bindings_from_path(path: &Path) -> LoadedSkillBindings {
    if let Ok(bytes) = fs::read(path) {
        if let Some(bindings) = decode(&bytes) {
            return LoadedSkillBindings {
                bindings,
                source: SkillBindingLoadSource::Primary,
            };
        }
    }

    if let Ok(bytes) = fs::read(backup_path(path)) {
        if let Some(bindings) = decode(&bytes) {
            return LoadedSkillBindings {
                bindings,
                source: SkillBindingLoadSource::Backup,
            };
        }
    }

    LoadedSkillBindings {
        bindings: SkillBindingUi::default(),
        source: SkillBindingLoadSource::Defaults,
    }
}

fn sync_parent_directory(parent: &Path) {
    // Directory fsync is useful after rename on Unix. Windows filesystems do
    // not all support opening a directory, so this is deliberately best effort.
    if let Ok(directory) = File::open(parent) {
        let _ = directory.sync_all();
    }
}

/// Write the allow-listed bindings through a same-directory temp file and
/// recoverable primary/backup replacement.
pub fn persist_skill_bindings_to_path(path: &Path, bindings: &SkillBindingUi) -> io::Result<()> {
    // SkillBindingUi's custom Serialize is the single allow-list authority:
    // selected_skill_id and assign_key cannot enter this payload.
    let payload = serde_json::to_vec_pretty(bindings).map_err(io::Error::other)?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;

    let temporary = temporary_path(path);
    let backup = backup_path(path);
    let write_result = (|| -> io::Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temporary)?;
        file.write_all(&payload)?;
        file.flush()?;
        file.sync_all()?;

        let had_primary = path.is_file();
        if had_primary {
            if backup.exists() {
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
                // If rotation happened, put the old primary back. If that
                // recovery fails, the backup remains available for next load.
                if had_primary && !path.exists() {
                    let _ = fs::rename(&backup, path);
                }
                Err(error)
            }
        }
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    write_result
}

fn normalized(bindings: &SkillBindingUi) -> io::Result<SkillBindingUi> {
    let bytes = serde_json::to_vec(bindings).map_err(io::Error::other)?;
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}

/// Startup system: load disk state into the renderer-neutral Bevy resource.
pub fn load_persisted_skill_bindings(
    mut bindings: ResMut<SkillBindingUi>,
    mut runtime: ResMut<SkillBindingPersistenceRuntime>,
) {
    let loaded = load_skill_bindings_from_path(&runtime.config_path);
    *bindings = loaded.bindings.clone();
    runtime.loaded_from = loaded.source;
    runtime.persisted_bindings = match loaded.source {
        SkillBindingLoadSource::Defaults => None,
        SkillBindingLoadSource::Primary | SkillBindingLoadSource::Backup => Some(loaded.bindings),
    };
    runtime.dirty = false;
    runtime.last_status = SkillBindingPersistStatus::NeverAttempted;
    runtime.last_error = None;
}

/// Persist the current binding set if it differs from the confirmed disk set.
/// Returns `true` for a successful write or an idempotent no-op, and `false`
/// only when the filesystem operation failed.
pub fn persist_skill_bindings_if_changed(
    runtime: &mut SkillBindingPersistenceRuntime,
    bindings: &SkillBindingUi,
) -> bool {
    let normalized = match normalized(bindings) {
        Ok(bindings) => bindings,
        Err(error) => {
            runtime.dirty = true;
            runtime.last_error = Some(error.to_string());
            runtime.last_status = SkillBindingPersistStatus::Failed(error.to_string());
            return false;
        }
    };

    if runtime.persisted_bindings.as_ref() == Some(&normalized) && runtime.config_path.is_file() {
        runtime.dirty = false;
        runtime.last_error = None;
        runtime.last_status = SkillBindingPersistStatus::SkippedUnchanged;
        return true;
    }

    match persist_skill_bindings_to_path(&runtime.config_path, &normalized) {
        Ok(()) => {
            runtime.persisted_bindings = Some(normalized);
            runtime.dirty = false;
            runtime.last_error = None;
            runtime.last_status = SkillBindingPersistStatus::Succeeded;
            true
        }
        Err(error) => {
            runtime.dirty = true;
            runtime.last_error = Some(error.to_string());
            runtime.last_status = SkillBindingPersistStatus::Failed(error.to_string());
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill_binding_ui::{SkillHotkeyBinding, MAX_SKILL_HOTKEY_BINDINGS};
    use bevy::prelude::App;
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
                "mir2-skill-bindings-{label}-{}-{id}",
                std::process::id()
            ));
            Self {
                file: root.join("config").join(SKILL_BINDINGS_FILE),
                root,
            }
        }
    }

    impl Drop for TestPath {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn sample() -> SkillBindingUi {
        SkillBindingUi {
            selected_skill_id: Some(999),
            assign_key: true,
            bindings: vec![
                SkillHotkeyBinding {
                    skill_id: 101,
                    hotkey: 1,
                },
                SkillHotkeyBinding {
                    skill_id: 202,
                    hotkey: 8,
                },
            ],
        }
    }

    #[test]
    fn roundtrip_uses_primary_and_resets_transient_ui_state() {
        let path = TestPath::new("roundtrip");
        let source = sample();
        persist_skill_bindings_to_path(&path.file, &source).expect("persist");

        let loaded = load_skill_bindings_from_path(&path.file);
        assert_eq!(loaded.source, SkillBindingLoadSource::Primary);
        assert_eq!(loaded.bindings.bindings, source.bindings);
        assert_eq!(loaded.bindings.selected_skill_id, None);
        assert!(!loaded.bindings.assign_key);
    }

    #[test]
    fn corrupt_primary_then_backup_then_defaults() {
        let path = TestPath::new("recovery");
        let first = sample();
        let second = SkillBindingUi {
            bindings: vec![SkillHotkeyBinding {
                skill_id: 303,
                hotkey: 4,
            }],
            ..SkillBindingUi::default()
        };
        persist_skill_bindings_to_path(&path.file, &first).expect("first persist");
        persist_skill_bindings_to_path(&path.file, &second).expect("second persist");
        fs::write(&path.file, b"not-json").expect("corrupt primary");

        let from_backup = load_skill_bindings_from_path(&path.file);
        assert_eq!(from_backup.source, SkillBindingLoadSource::Backup);
        assert_eq!(from_backup.bindings.bindings, first.bindings);

        fs::write(backup_path(&path.file), b"also-not-json").expect("corrupt backup");
        let from_defaults = load_skill_bindings_from_path(&path.file);
        assert_eq!(from_defaults.source, SkillBindingLoadSource::Defaults);
        assert!(from_defaults.bindings.bindings.is_empty());
    }

    #[test]
    fn serialized_payload_is_only_the_bindings_allow_list() {
        let path = TestPath::new("allow-list");
        persist_skill_bindings_to_path(&path.file, &sample()).expect("persist");
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path.file).expect("payload")).expect("json");
        let object = value.as_object().expect("object payload");
        assert_eq!(object.keys().cloned().collect::<Vec<_>>(), vec!["bindings"]);
        assert!(object.get("selectedSkillId").is_none());
        assert!(object.get("assignKey").is_none());
        assert_eq!(object["bindings"].as_array().unwrap().len(), 2);
        assert!(fs::read_to_string(&path.file)
            .expect("payload text")
            .find("password")
            .is_none());
    }

    #[test]
    fn runtime_persist_is_idempotent_and_exposes_success() {
        let path = TestPath::new("idempotent");
        let mut runtime = SkillBindingPersistenceRuntime::with_config_path(path.file.clone());
        let source = sample();

        assert!(persist_skill_bindings_if_changed(&mut runtime, &source));
        assert_eq!(runtime.last_status, SkillBindingPersistStatus::Succeeded);
        let first_bytes = fs::read(&path.file).expect("primary");

        assert!(persist_skill_bindings_if_changed(&mut runtime, &source));
        assert_eq!(
            runtime.last_status,
            SkillBindingPersistStatus::SkippedUnchanged
        );
        assert_eq!(fs::read(&path.file).expect("primary"), first_bytes);
        assert!(!runtime.dirty);
    }

    #[test]
    fn failed_persist_is_observable_and_does_not_commit_cache() {
        let path = TestPath::new("failure");
        fs::create_dir_all(&path.file).expect("make target a directory");
        let mut runtime = SkillBindingPersistenceRuntime::with_config_path(path.file.clone());
        runtime.mark_dirty();
        let source = sample();

        assert!(!persist_skill_bindings_if_changed(&mut runtime, &source));
        assert!(matches!(
            runtime.last_status,
            SkillBindingPersistStatus::Failed(_)
        ));
        assert!(runtime.last_error.is_some());
        assert!(runtime.dirty);
        assert!(runtime.persisted_bindings.is_none());
        assert!(!temporary_path(&path.file).exists());
    }

    #[test]
    fn startup_system_loads_resource_and_plugin_registers_resources() {
        let path = TestPath::new("startup");
        let source = sample();
        persist_skill_bindings_to_path(&path.file, &source).expect("persist");

        let mut app = App::new();
        app.init_resource::<SkillBindingUi>()
            .insert_resource(SkillBindingPersistenceRuntime::with_config_path(
                path.file.clone(),
            ))
            .add_systems(Startup, load_persisted_skill_bindings);
        app.update();

        let loaded = app.world().resource::<SkillBindingUi>();
        assert_eq!(loaded.bindings, source.bindings);
        assert_eq!(loaded.selected_skill_id, None);
        assert_eq!(
            app.world()
                .resource::<SkillBindingPersistenceRuntime>()
                .loaded_from,
            SkillBindingLoadSource::Primary
        );

        let mut plugin_app = App::new();
        plugin_app.add_plugins(SkillBindingPersistencePlugin);
        assert!(plugin_app.world().contains_resource::<SkillBindingUi>());
        assert!(plugin_app
            .world()
            .contains_resource::<SkillBindingPersistenceRuntime>());
    }

    #[test]
    fn deserialize_keeps_the_existing_bounded_sanitizing_contract() {
        let mut bindings = Vec::new();
        for skill_id in 1..=32 {
            bindings.push(serde_json::json!({
                "skillId": skill_id,
                "hotkey": skill_id
            }));
        }
        let raw = serde_json::json!({
            "bindings": bindings,
            "selectedSkillId": 123,
            "assignKey": true
        });
        let path = TestPath::new("sanitize");
        fs::create_dir_all(path.file.parent().unwrap()).expect("parent");
        fs::write(&path.file, serde_json::to_vec(&raw).unwrap()).expect("write");

        let loaded = load_skill_bindings_from_path(&path.file);
        assert_eq!(loaded.bindings.bindings.len(), MAX_SKILL_HOTKEY_BINDINGS);
        assert_eq!(loaded.bindings.skill_for_hotkey(1), Some(1));
        assert_eq!(loaded.bindings.skill_for_hotkey(8), Some(8));
        assert_eq!(loaded.bindings.selected_skill_id, None);
        assert!(!loaded.bindings.assign_key);
    }
}
