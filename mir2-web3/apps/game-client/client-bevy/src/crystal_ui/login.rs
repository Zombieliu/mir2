//! Crystal login-screen composition for the native Bevy shell.

use bevy::prelude::*;
use bevy::ui::{AlignItems, BackgroundColor, JustifyContent, Node, PositionType, UiRect, Val};

use crate::native_shell::{LoginFocus, NativeShellModel};

use super::assets::login_assets;
use super::spec::{self, CrystalFrameSpec, CrystalRect};
use super::widget::spawn_crystal_image_button;

const FIELD_BG: Color = Color::BLACK;
const FIELD_TEXT: Color = Color::srgb(0.96, 0.94, 0.90);
const NOTICE_INFO: Color = Color::srgb(0.94, 0.88, 0.72);
const NOTICE_ERROR: Color = Color::srgb(0.93, 0.47, 0.43);
const CARET_BLINK_SECONDS: f32 = 0.5;

#[derive(Component)]
pub struct CrystalLoginCaret {
    timer: Timer,
}

impl Default for CrystalLoginCaret {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(CARET_BLINK_SECONDS, TimerMode::Repeating),
        }
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrystalLoginAction {
    FocusAccount,
    FocusPassword,
    Login,
    RegisterAccount,
    ChangePassword,
    SafeKey,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginElementKind {
    Background,
    Panel,
    Title,
    AccountLabel,
    PasswordLabel,
    AccountField,
    PasswordField,
    OkButton,
    NewAccountButton,
    ChangePasswordButton,
    SafeKeyButton,
    CancelButton,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoginElementSpec {
    pub kind: LoginElementKind,
    pub rect: CrystalRect,
}

pub const LOGIN_ELEMENT_SPECS: [LoginElementSpec; 12] = [
    LoginElementSpec {
        kind: LoginElementKind::Background,
        rect: spec::login::BACKGROUND.rect,
    },
    LoginElementSpec {
        kind: LoginElementKind::Panel,
        rect: spec::login::PANEL.rect,
    },
    LoginElementSpec {
        kind: LoginElementKind::Title,
        rect: spec::login::TITLE.rect,
    },
    LoginElementSpec {
        kind: LoginElementKind::AccountLabel,
        rect: spec::login::ACCOUNT_LABEL.rect,
    },
    LoginElementSpec {
        kind: LoginElementKind::PasswordLabel,
        rect: spec::login::PASSWORD_LABEL.rect,
    },
    LoginElementSpec {
        kind: LoginElementKind::AccountField,
        rect: spec::login::ACCOUNT_FIELD,
    },
    LoginElementSpec {
        kind: LoginElementKind::PasswordField,
        rect: spec::login::PASSWORD_FIELD,
    },
    LoginElementSpec {
        kind: LoginElementKind::OkButton,
        rect: spec::login::OK.rect,
    },
    LoginElementSpec {
        kind: LoginElementKind::NewAccountButton,
        rect: spec::login::NEW_ACCOUNT.rect,
    },
    LoginElementSpec {
        kind: LoginElementKind::ChangePasswordButton,
        rect: spec::login::CHANGE_PASSWORD.rect,
    },
    LoginElementSpec {
        kind: LoginElementKind::SafeKeyButton,
        rect: spec::login::SAFE_KEY.rect,
    },
    LoginElementSpec {
        kind: LoginElementKind::CancelButton,
        rect: spec::login::CANCEL.rect,
    },
];

pub fn spawn_login_screen(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    model: &NativeShellModel,
    masked_password: &str,
) {
    let assets = login_assets();
    spawn_stage_frame(parent, asset_server, spec::login::PANEL);
    spawn_stage_frame(parent, asset_server, spec::login::TITLE);
    spawn_stage_frame(parent, asset_server, spec::login::ACCOUNT_LABEL);
    spawn_stage_frame(parent, asset_server, spec::login::PASSWORD_LABEL);

    spawn_login_field(
        parent,
        spec::login::ACCOUNT_FIELD,
        &model.login.account,
        matches!(model.login.focus, LoginFocus::Account),
        CrystalLoginAction::FocusAccount,
    );
    spawn_login_field(
        parent,
        spec::login::PASSWORD_FIELD,
        masked_password,
        matches!(model.login.focus, LoginFocus::Password),
        CrystalLoginAction::FocusPassword,
    );

    spawn_crystal_image_button(
        parent,
        asset_server,
        spec::login::OK,
        assets.ok,
        CrystalLoginAction::Login,
        matches!(model.login.focus, LoginFocus::LoginButton),
        model.login.is_ready(),
    );
    spawn_crystal_image_button(
        parent,
        asset_server,
        spec::login::NEW_ACCOUNT,
        assets.new_account,
        CrystalLoginAction::RegisterAccount,
        matches!(model.login.focus, LoginFocus::NewAccountButton),
        model.login.is_ready(),
    );
    spawn_crystal_image_button(
        parent,
        asset_server,
        spec::login::CHANGE_PASSWORD,
        assets.change_password,
        CrystalLoginAction::ChangePassword,
        false,
        true,
    );
    spawn_crystal_image_button(
        parent,
        asset_server,
        spec::login::SAFE_KEY,
        assets.safe_key,
        CrystalLoginAction::SafeKey,
        false,
        true,
    );
    spawn_crystal_image_button(
        parent,
        asset_server,
        spec::login::CANCEL,
        assets.cancel,
        CrystalLoginAction::Cancel,
        false,
        true,
    );

    if let Some(notice) = &model.notice {
        spawn_notice(
            parent,
            notice.message.as_str(),
            matches!(notice.kind, crate::native_shell::ShellNoticeKind::Error),
        );
    }
}

fn spawn_stage_frame(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    frame: CrystalFrameSpec,
) {
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(frame.rect.left),
            top: Val::Px(frame.rect.top),
            width: Val::Px(frame.rect.width),
            height: Val::Px(frame.rect.height),
            ..default()
        },
        ImageNode {
            image: asset_server.load(frame.asset_path()),
            ..default()
        },
    ));
}

fn spawn_login_field(
    parent: &mut ChildSpawnerCommands,
    rect: CrystalRect,
    text: &str,
    focused: bool,
    action: CrystalLoginAction,
) {
    parent
        .spawn((
            Button,
            action,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(rect.left),
                top: Val::Px(rect.top),
                width: Val::Px(rect.width),
                height: Val::Px(rect.height),
                padding: UiRect::horizontal(Val::Px(2.0)),
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(FIELD_BG),
        ))
        .with_children(|field| {
            field.spawn((
                Text::new(text.to_owned()),
                TextFont {
                    font_size: FontSize::Px(10.0),
                    ..default()
                },
                TextColor(FIELD_TEXT),
            ));
            if focused {
                field.spawn((
                    CrystalLoginCaret::default(),
                    Text::new("|"),
                    TextFont {
                        font_size: FontSize::Px(10.0),
                        ..default()
                    },
                    TextColor(FIELD_TEXT),
                    Visibility::Visible,
                ));
            }
        });
}

pub fn blink_login_caret(
    time: Res<Time>,
    mut carets: Query<(&mut CrystalLoginCaret, &mut Visibility)>,
) {
    for (mut caret, mut visibility) in &mut carets {
        caret.timer.tick(time.delta());
        if caret.timer.just_finished() {
            *visibility = match *visibility {
                Visibility::Hidden => Visibility::Visible,
                _ => Visibility::Hidden,
            };
        }
    }
}

fn spawn_notice(parent: &mut ChildSpawnerCommands, text: &str, is_error: bool) {
    parent
        .spawn((Node {
            position_type: PositionType::Absolute,
            left: Val::Px(348.0),
            top: Val::Px(502.0),
            width: Val::Px(328.0),
            justify_content: JustifyContent::Center,
            ..default()
        },))
        .with_children(|notice| {
            notice.spawn((
                Text::new(text.to_owned()),
                TextFont {
                    font_size: FontSize::Px(15.0),
                    ..default()
                },
                TextColor(if is_error { NOTICE_ERROR } else { NOTICE_INFO }),
            ));
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_element_set_covers_expected_crystal_scene_members() {
        assert_eq!(LOGIN_ELEMENT_SPECS.len(), 12);
        assert_eq!(LOGIN_ELEMENT_SPECS[0].kind, LoginElementKind::Background);
        assert_eq!(
            LOGIN_ELEMENT_SPECS[1].rect,
            CrystalRect::new(348.0, 274.0, 328.0, 220.0)
        );
        assert!(LOGIN_ELEMENT_SPECS
            .iter()
            .any(|element| element.kind == LoginElementKind::CancelButton
                && element.rect == spec::login::CANCEL.rect));
    }

    #[test]
    fn login_button_specs_use_exact_crystal_hit_rects() {
        let buttons: [super::super::spec::CrystalButtonSpec; 5] = [
            spec::login::OK,
            spec::login::NEW_ACCOUNT,
            spec::login::CHANGE_PASSWORD,
            spec::login::SAFE_KEY,
            spec::login::CANCEL,
        ];
        assert_eq!(buttons[0].rect, CrystalRect::new(575.0, 355.0, 42.0, 42.0));
        assert_eq!(buttons[1].rect.top, 437.0);
        assert_eq!(buttons[4].rect.left, 514.0);
    }

    #[test]
    fn login_caret_uses_crystal_style_half_second_blink() {
        let caret = CrystalLoginCaret::default();
        assert_eq!(caret.timer.duration().as_secs_f32(), CARET_BLINK_SECONDS);
        assert_eq!(caret.timer.mode(), TimerMode::Repeating);
    }
}
