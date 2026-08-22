//! Native Windows screenshot plugin for the 1024x768 native Bevy slice.
//!
//! The plugin is intentionally off by default. It only enables capture when
//! `MIR2_NATIVE_CAPTURE_DIR` is provided with a valid, non-empty value.

use bevy::{
    input::ButtonInput,
    prelude::{App, KeyCode, Plugin, Res, ResMut, Resource, Update},
    render::view::screenshot::{save_to_disk, Screenshot},
};
use mir2_client_bevy::native_shell::{NativeShellModel, NativeShellScreen};
use mir2_client_bevy::quest_model::{CombatTargetModel, QuestStatus, QuestTracker};
use std::{
    env,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::SystemTime,
};

const ENV_CAPTURE_DIR: &str = "MIR2_NATIVE_CAPTURE_DIR";
const ENV_CAPTURE_LABEL: &str = "MIR2_NATIVE_CAPTURE_LABEL";
const ENV_CAPTURE_AUTO_SCREEN: &str = "MIR2_NATIVE_CAPTURE_AUTO_SCREEN";
const ENV_CAPTURE_QUEST_INDEX: &str = "MIR2_NATIVE_CAPTURE_QUEST_INDEX";
const AUTO_CAPTURE_WAIT_FRAMES: u32 = 60;
const DEFAULT_STABLE_SLUG: &str = "unknown";
const DEFAULT_LABEL_PREFIX: &str = "mir2";
static CAPTURE_TRACE_FRAME: AtomicU64 = AtomicU64::new(0);

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
) {
    if !keys.just_pressed(KeyCode::F12) {
        return;
    }

    let screen_slug = shell
        .as_deref()
        .map(|model| native_shell_screen_slug(model.screen))
        .unwrap_or(DEFAULT_STABLE_SLUG);
    let path = runtime.auto_capture_path(&config, screen_slug);
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path));
}

fn auto_capture_system(
    mut commands: bevy::prelude::Commands,
    config: Res<NativeCaptureConfig>,
    mut runtime: ResMut<NativeCaptureRuntime>,
    shell: Option<Res<NativeShellModel>>,
    tracker: Option<Res<QuestTracker>>,
    combat: Option<Res<CombatTargetModel>>,
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
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }
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

fn unix_time_millis() -> u128 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |now| now.as_millis())
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
}
