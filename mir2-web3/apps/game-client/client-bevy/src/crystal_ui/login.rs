//! Crystal login-screen composition for the native Bevy shell.

use bevy::prelude::*;
use bevy::ui::{AlignItems, BackgroundColor, JustifyContent, Node, PositionType, UiRect, Val};

use crate::native_login_locale::{NativeLoginCaption, NativeUiLocale};
use crate::native_shell::{LoginFocus, NativeShellModel};

use super::assets::login_assets;
use super::spec::{self, CrystalFrameSpec, CrystalRect};
use super::widget::spawn_crystal_image_button;

const FIELD_BG: Color = Color::BLACK;
const FIELD_TEXT: Color = Color::srgb(0.96, 0.94, 0.90);
const NOTICE_INFO: Color = Color::srgb(0.94, 0.88, 0.72);
const NOTICE_ERROR: Color = Color::srgb(0.93, 0.47, 0.43);
const CARET_BLINK_SECONDS: f32 = 0.5;
const WALLET_LOGIN_BUTTON_RECT: CrystalRect = CrystalRect::new(378.0, 550.0, 268.0, 34.0);

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
    DubheWallet,
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
    locale: NativeUiLocale,
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
        true,
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

    parent
        .spawn((
            Button,
            CrystalLoginAction::DubheWallet,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(WALLET_LOGIN_BUTTON_RECT.left),
                top: Val::Px(WALLET_LOGIN_BUTTON_RECT.top),
                width: Val::Px(WALLET_LOGIN_BUTTON_RECT.width),
                height: Val::Px(WALLET_LOGIN_BUTTON_RECT.height),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(Color::srgb(0.67, 0.51, 0.24)),
            BackgroundColor(Color::srgba(0.12, 0.10, 0.07, 0.96)),
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(locale.wallet_entry()),
                TextFont {
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(NOTICE_INFO),
            ));
        });

    // Original bitmap labels contain baked-in text. Cover only their caption
    // areas, leaving every existing button and field hit rectangle unchanged.
    for (rect, caption, size) in [
        (
            CrystalRect::new(368.0, 286.0, 288.0, 28.0),
            NativeLoginCaption::Title,
            16.0,
        ),
        (
            CrystalRect::new(352.0, 355.0, 77.0, 21.0),
            NativeLoginCaption::Account,
            10.0,
        ),
        (
            CrystalRect::new(352.0, 378.0, 77.0, 21.0),
            NativeLoginCaption::Password,
            10.0,
        ),
        (spec::login::OK.rect, NativeLoginCaption::Login, 11.0),
        (
            spec::login::NEW_ACCOUNT.rect,
            NativeLoginCaption::NewAccount,
            10.0,
        ),
        (
            spec::login::CHANGE_PASSWORD.rect,
            NativeLoginCaption::ChangePassword,
            10.0,
        ),
        (
            spec::login::SAFE_KEY.rect,
            NativeLoginCaption::SafeKey,
            10.0,
        ),
        (spec::login::CANCEL.rect, NativeLoginCaption::Cancel, 10.0),
    ] {
        spawn_localized_caption(
            parent,
            rect,
            &locale.login_caption(caption),
            size,
            caption != NativeLoginCaption::Login || model.login.is_ready(),
        );
    }

    if let Some(notice) = &model.notice {
        spawn_notice(
            parent,
            notice.message.as_str(),
            matches!(notice.kind, crate::native_shell::ShellNoticeKind::Error),
        );
    }
}

fn spawn_localized_caption(
    parent: &mut ChildSpawnerCommands,
    rect: CrystalRect,
    caption: &str,
    font_size: f32,
    enabled: bool,
) {
    parent
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(rect.left + 1.0),
                top: Val::Px(rect.top + 1.0),
                width: Val::Px(rect.width - 2.0),
                height: Val::Px(rect.height - 2.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.09, 0.07, 0.05)),
            bevy::ui::FocusPolicy::Pass,
            ZIndex(1),
        ))
        .with_children(|label| {
            label.spawn((
                Text::new(caption),
                TextFont {
                    font_size: FontSize::Px(font_size),
                    ..default()
                },
                TextColor(if enabled {
                    NOTICE_INFO
                } else {
                    Color::srgba(0.94, 0.88, 0.72, 0.45)
                }),
                bevy::ui::FocusPolicy::Pass,
            ));
        });
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
    fn wallet_entry_does_not_overlap_classic_login_controls_or_notice() {
        for element in LOGIN_ELEMENT_SPECS.iter().filter(|element| {
            matches!(
                element.kind,
                LoginElementKind::AccountField
                    | LoginElementKind::PasswordField
                    | LoginElementKind::OkButton
                    | LoginElementKind::NewAccountButton
                    | LoginElementKind::ChangePasswordButton
                    | LoginElementKind::SafeKeyButton
                    | LoginElementKind::CancelButton
            )
        }) {
            assert!(
                WALLET_LOGIN_BUTTON_RECT.top >= element.rect.top + element.rect.height,
                "wallet button overlaps {:?}",
                element.kind
            );
        }
        assert!(WALLET_LOGIN_BUTTON_RECT.top >= 540.0);
        assert!(WALLET_LOGIN_BUTTON_RECT.top + WALLET_LOGIN_BUTTON_RECT.height <= 768.0);
    }

    #[test]
    fn login_caret_uses_crystal_style_half_second_blink() {
        let caret = CrystalLoginCaret::default();
        assert_eq!(caret.timer.duration().as_secs_f32(), CARET_BLINK_SECONDS);
        assert_eq!(caret.timer.mode(), TimerMode::Repeating);
    }
}
