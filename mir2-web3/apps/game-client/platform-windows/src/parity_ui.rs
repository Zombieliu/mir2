use std::fs;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::ui::{
    AlignItems, BackgroundColor, BorderColor, Display, FlexDirection, JustifyContent, Node,
    PositionType, UiRect, Val,
};
use mir2_client_bevy::chat::{ChatChannel, ChatLine, ChatModel};
use mir2_client_bevy::native_shell::{NativeShellModel, NativeShellScreen};
use serde::Deserialize;

const SHELL_OVERLAY_Z: i32 = 1_010;
const BULLETIN_OVERLAY_Z: i32 = 990;
const BULLETIN_MAX_LINES: usize = 8;
const BUILD_TEXT: Color = Color::srgba(0.96, 0.94, 0.88, 0.96);
const BUILD_SHADOW: Color = Color::srgba(0.0, 0.0, 0.0, 0.92);
const BADGE_BG: Color = Color::srgba(0.96, 0.48, 0.06, 0.98);
const BADGE_TEXT: Color = Color::WHITE;
const PANEL_BG: Color = Color::srgba(0.04, 0.04, 0.05, 0.95);
const PANEL_BORDER: Color = Color::srgb(0.79, 0.67, 0.44);
const PANEL_TEXT: Color = Color::srgb(0.92, 0.90, 0.84);
const PANEL_LINK: Color = Color::srgb(0.98, 0.91, 0.17);
const BUTTON_BG: Color = Color::srgba(0.29, 0.19, 0.11, 1.0);
const BUTTON_BORDER: Color = Color::srgb(0.73, 0.60, 0.38);

#[derive(Debug, Clone, PartialEq, Eq, Resource)]
pub struct NativeBuildStamp {
    pub version_label: String,
    pub git_revision_short: String,
    pub platform_label: String,
    pub mode_badge: Option<String>,
}

impl NativeBuildStamp {
    pub fn discover() -> Self {
        let metadata = discover_build_metadata();
        let candidate = metadata.candidate.clone();
        let git_revision_short = metadata
            .git_revision
            .as_deref()
            .map(short_git_revision)
            .unwrap_or_else(|| "unknown".to_owned());
        let version_label = candidate
            .clone()
            .or(metadata.exe_name)
            .unwrap_or_else(|| "local-dev".to_owned());
        let mode_badge = candidate
            .as_deref()
            .map(|candidate| candidate.to_ascii_uppercase())
            .or_else(|| metadata.debug_build.then(|| "TEST MODE".to_owned()));

        Self {
            version_label,
            git_revision_short,
            platform_label: platform_label(),
            mode_badge,
        }
    }

    fn build_string(&self) -> String {
        format!(
            "Build: {} · {} · {}",
            self.version_label, self.git_revision_short, self.platform_label
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Resource)]
struct LoginAnnouncementState {
    visible: bool,
    dismissed: bool,
    was_in_game: bool,
    chat_cursor: usize,
    lines: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ShellOverlaySnapshot {
    shell_screen: Option<NativeShellScreen>,
    build: Option<NativeBuildStamp>,
}

#[derive(Component)]
struct ShellBuildOverlayRoot;

#[derive(Component)]
struct BulletinOverlayRoot;

#[derive(Component)]
enum BulletinButton {
    Dismiss,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct VersionMetadata {
    candidate: Option<String>,
    git_revision: Option<String>,
    git_revision_legacy: Option<String>,
    exe_name: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct BuildMetadata {
    candidate: Option<String>,
    git_revision: Option<String>,
    exe_name: Option<String>,
    debug_build: bool,
}

pub struct Mir2NativeParityUiPlugin;

impl Plugin for Mir2NativeParityUiPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(NativeBuildStamp::discover())
            .init_resource::<LoginAnnouncementState>()
            .add_systems(Startup, spawn_parity_ui_roots)
            .add_systems(
                Update,
                (
                    update_login_announcement_state,
                    bulletin_button_input,
                    render_shell_build_overlay,
                    render_login_announcement,
                )
                    .chain(),
            );
    }
}

fn spawn_parity_ui_roots(mut commands: Commands) {
    commands.spawn((
        ShellBuildOverlayRoot,
        Node {
            width: Val::Px(1024.0),
            height: Val::Px(768.0),
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            display: Display::None,
            ..default()
        },
        GlobalZIndex(SHELL_OVERLAY_Z),
    ));

    commands.spawn((
        BulletinOverlayRoot,
        Node {
            width: Val::Px(1024.0),
            height: Val::Px(768.0),
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            display: Display::None,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        GlobalZIndex(BULLETIN_OVERLAY_Z),
    ));
}

fn render_shell_build_overlay(
    mut commands: Commands,
    shell: Option<Res<NativeShellModel>>,
    build: Option<Res<NativeBuildStamp>>,
    roots: Query<Entity, With<ShellBuildOverlayRoot>>,
    mut last_rendered: Local<Option<ShellOverlaySnapshot>>,
) {
    let Ok(root) = roots.single() else {
        return;
    };
    let snapshot = ShellOverlaySnapshot {
        shell_screen: shell.as_deref().map(|shell| shell.screen),
        build: build.as_deref().cloned(),
    };
    if last_rendered.as_ref() == Some(&snapshot) {
        return;
    }
    *last_rendered = Some(snapshot.clone());

    let Some(shell) = shell else {
        commands.entity(root).insert(Node {
            display: Display::None,
            ..default()
        });
        commands.entity(root).despawn_children();
        return;
    };

    let visible = shell.screen != NativeShellScreen::InGame;
    commands.entity(root).insert(Node {
        width: Val::Px(1024.0),
        height: Val::Px(768.0),
        position_type: PositionType::Absolute,
        left: Val::Px(0.0),
        top: Val::Px(0.0),
        display: if visible { Display::Flex } else { Display::None },
        ..default()
    });
    commands.entity(root).despawn_children();
    if !visible {
        return;
    }
    let Some(build) = build else {
        return;
    };

    commands.entity(root).with_children(|parent| {
        parent.spawn((
            Text::new(build.build_string()),
            TextFont {
                font_size: FontSize::Px(10.0),
                ..default()
            },
            TextColor(BUILD_SHADOW),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(5.0),
                bottom: Val::Px(4.0),
                ..default()
            },
        ));
        parent.spawn((
            Text::new(build.build_string()),
            TextFont {
                font_size: FontSize::Px(10.0),
                ..default()
            },
            TextColor(BUILD_TEXT),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(4.0),
                bottom: Val::Px(5.0),
                ..default()
            },
        ));

        if let Some(mode_badge) = build.mode_badge.as_deref() {
            parent
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        right: Val::Px(16.0),
                        top: Val::Px(16.0),
                        padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    BackgroundColor(BADGE_BG),
                    BorderColor::all(Color::WHITE),
                ))
                .with_children(|badge| {
                    badge.spawn((
                        Text::new(mode_badge.to_owned()),
                        TextFont {
                            font_size: FontSize::Px(18.0),
                            ..default()
                        },
                        TextColor(BADGE_TEXT),
                    ));
                });
        }
    });
}

fn update_login_announcement_state(
    shell: Option<Res<NativeShellModel>>,
    chat: Option<Res<ChatModel>>,
    mut state: ResMut<LoginAnnouncementState>,
) {
    let shell_screen = shell
        .as_deref()
        .map(|shell| shell.screen)
        .unwrap_or(NativeShellScreen::Login);
    let chat_lines = chat.as_deref().map(|model| model.lines.as_slice()).unwrap_or(&[]);

    if shell_screen != NativeShellScreen::InGame {
        if state.was_in_game {
            *state = LoginAnnouncementState {
                chat_cursor: chat_lines.len(),
                ..default()
            };
        } else if state.chat_cursor == 0 {
            state.chat_cursor = chat_lines.len();
        }
        state.was_in_game = false;
        state.visible = false;
        return;
    }

    let start = state.chat_cursor.min(chat_lines.len());
    for line in &chat_lines[start..] {
        if !is_bulletin_line(line) {
            continue;
        }
        let text = line.text.trim();
        if text.is_empty() || state.lines.iter().any(|existing| existing == text) {
            continue;
        }
        state.lines.push(text.to_owned());
        if state.lines.len() > BULLETIN_MAX_LINES {
            let overflow = state.lines.len() - BULLETIN_MAX_LINES;
            state.lines.drain(0..overflow);
        }
    }
    state.chat_cursor = chat_lines.len();
    state.was_in_game = true;
    if !state.dismissed && !state.lines.is_empty() {
        state.visible = true;
    }
}

fn render_login_announcement(
    mut commands: Commands,
    state: Res<LoginAnnouncementState>,
    roots: Query<Entity, With<BulletinOverlayRoot>>,
    mut last_rendered: Local<Option<LoginAnnouncementState>>,
) {
    let Ok(root) = roots.single() else {
        return;
    };
    if last_rendered.as_ref() == Some(state.as_ref()) {
        return;
    }
    *last_rendered = Some(state.clone());

    commands.entity(root).insert(Node {
        width: Val::Px(1024.0),
        height: Val::Px(768.0),
        position_type: PositionType::Absolute,
        left: Val::Px(0.0),
        top: Val::Px(0.0),
        display: if state.visible { Display::Flex } else { Display::None },
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    });
    commands.entity(root).despawn_children();
    if !state.visible {
        return;
    }

    commands.entity(root).with_children(|overlay| {
        overlay
            .spawn((
                Node {
                    width: Val::Px(314.0),
                    min_height: Val::Px(434.0),
                    border: UiRect::all(Val::Px(2.0)),
                    padding: UiRect::all(Val::Px(12.0)),
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                BackgroundColor(PANEL_BG),
                BorderColor::all(PANEL_BORDER),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("Welcome to Legend of Mir 2"),
                    TextFont {
                        font_size: FontSize::Px(18.0),
                        ..default()
                    },
                    TextColor(PANEL_TEXT),
                    Node {
                        margin: UiRect::bottom(Val::Px(10.0)),
                        ..default()
                    },
                ));

                panel
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        min_height: Val::Px(332.0),
                        padding: UiRect::all(Val::Px(10.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(8.0),
                        ..default()
                    })
                    .insert(BackgroundColor(Color::srgba(0.02, 0.03, 0.06, 0.92)))
                    .insert(BorderColor::all(Color::srgba(0.65, 0.58, 0.40, 0.9)))
                    .with_children(|body| {
                        for line in &state.lines {
                            body.spawn((
                                Text::new(line.clone()),
                                TextFont {
                                    font_size: FontSize::Px(16.0),
                                    ..default()
                                },
                                TextColor(bulletin_line_color(line)),
                            ));
                        }
                        body.spawn((
                            Text::new(
                                "By clicking close and continuing to play the game you are agreeing to the terms of service above.",
                            ),
                            TextFont {
                                font_size: FontSize::Px(14.0),
                                ..default()
                            },
                            TextColor(PANEL_TEXT),
                            Node {
                                margin: UiRect::top(Val::Px(16.0)),
                                ..default()
                            },
                        ));
                    });

                panel
                    .spawn((
                        Button,
                        BulletinButton::Dismiss,
                        Node {
                            width: Val::Px(94.0),
                            height: Val::Px(28.0),
                            margin: UiRect::top(Val::Px(14.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            align_self: AlignSelf::Center,
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(BUTTON_BG),
                        BorderColor::all(BUTTON_BORDER),
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new("CLOSE"),
                            TextFont {
                                font_size: FontSize::Px(15.0),
                                ..default()
                            },
                            TextColor(PANEL_TEXT),
                        ));
                    });
            });
    });
}

fn bulletin_button_input(
    mut interactions: Query<(&Interaction, &BulletinButton), (Changed<Interaction>, With<Button>)>,
    mut state: ResMut<LoginAnnouncementState>,
) {
    for (interaction, button) in &mut interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button {
            BulletinButton::Dismiss => {
                state.visible = false;
                state.dismissed = true;
            }
        }
    }
}

fn is_bulletin_line(line: &ChatLine) -> bool {
    match line.canonical_channel() {
        ChatChannel::LineMessage => true,
        ChatChannel::Hint => {
            let text = line.text.trim();
            text.eq_ignore_ascii_case("Welcome to the Legend of Mir 2 Server.")
                || text.starts_with("Welcome to ")
        }
        _ => false,
    }
}

fn bulletin_line_color(line: &str) -> Color {
    let lower = line.to_ascii_lowercase();
    if lower.contains("lomcn") || lower.contains("database") || lower.contains("github") {
        PANEL_LINK
    } else {
        PANEL_TEXT
    }
}

fn discover_build_metadata() -> BuildMetadata {
    for path in version_metadata_candidates() {
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        let Ok(parsed) = serde_json::from_slice::<VersionMetadata>(&bytes) else {
            continue;
        };
        return BuildMetadata {
            candidate: parsed.candidate,
            git_revision: parsed.git_revision.or(parsed.git_revision_legacy),
            exe_name: parsed.exe_name,
            debug_build: cfg!(debug_assertions),
        };
    }

    BuildMetadata {
        debug_build: cfg!(debug_assertions),
        ..default()
    }
}

fn version_metadata_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("VERSION.json"));
            for ancestor in dir.ancestors() {
                candidates.push(ancestor.join("dist/mir2-windows-candidate/VERSION.json"));
                candidates.push(ancestor.join("mir2-web3/dist/mir2-windows-candidate/VERSION.json"));
            }
        }
    }
    if let Ok(current_dir) = std::env::current_dir() {
        for ancestor in current_dir.ancestors() {
            candidates.push(ancestor.join("dist/mir2-windows-candidate/VERSION.json"));
            candidates.push(ancestor.join("mir2-web3/dist/mir2-windows-candidate/VERSION.json"));
        }
    }
    dedupe_paths(candidates)
}

fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut unique: Vec<PathBuf> = Vec::new();
    for path in paths {
        if unique.iter().any(|existing| same_path(existing, &path)) {
            continue;
        }
        unique.push(path);
    }
    unique
}

fn same_path(left: &Path, right: &Path) -> bool {
    left == right
}

fn short_git_revision(value: &str) -> String {
    value.chars().take(7).collect()
}

fn platform_label() -> String {
    let platform = if cfg!(target_os = "windows") {
        "Windows"
    } else if cfg!(target_os = "macos") {
        "macOS"
    } else if cfg!(target_os = "linux") {
        "Linux"
    } else {
        std::env::consts::OS
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        other => other,
    };
    format!("{platform} {arch}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bulletin_accepts_line_messages_and_welcome_hint_only() {
        assert!(is_bulletin_line(&ChatLine {
            text: "www.LOMCN.net".to_owned(),
            channel: "LineMessage".to_owned(),
        }));
        assert!(is_bulletin_line(&ChatLine {
            text: "Welcome to the Legend of Mir 2 Server.".to_owned(),
            channel: "Hint".to_owned(),
        }));
        assert!(!is_bulletin_line(&ChatLine {
            text: "Online Players: 3".to_owned(),
            channel: "Hint".to_owned(),
        }));
    }

    #[test]
    fn build_stamp_uses_candidate_when_present() {
        let stamp = NativeBuildStamp {
            version_label: "WN-CANDIDATE-01".to_owned(),
            git_revision_short: "119553f".to_owned(),
            platform_label: "Windows x64".to_owned(),
            mode_badge: Some("WN-CANDIDATE-01".to_owned()),
        };
        assert_eq!(
            stamp.build_string(),
            "Build: WN-CANDIDATE-01 · 119553f · Windows x64"
        );
    }
}
