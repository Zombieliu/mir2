//! Windows wallet-first presentation. Every control retains its native action.
//! This is code-rendered UI, not a flattened mockup or authentication substitute.
use super::super::spec::CrystalRect;
use super::{login_font, CrystalLoginAction, CrystalLoginCaret};
use crate::native_login_locale::{NativeLoginCaption as Caption, NativeUiLocale};
use crate::native_shell::{LoginFocus, NativeShellModel, ShellNoticeKind};
use bevy::prelude::*;

const INK: Color = Color::srgb(0.035, 0.042, 0.047);
const PANEL: Color = Color::srgb(0.07, 0.078, 0.082);
const EDGE: Color = Color::srgb(0.40, 0.34, 0.23);
const GOLD: Color = Color::srgb(0.75, 0.64, 0.42);
const TEXT: Color = Color::srgb(0.91, 0.89, 0.83);
const MUTED: Color = Color::srgb(0.61, 0.63, 0.62);
const WALLET: CrystalRect = CrystalRect::new(582.0, 206.0, 348.0, 52.0);
const ACCOUNT: CrystalRect = CrystalRect::new(582.0, 365.0, 348.0, 32.0);
const PASSWORD: CrystalRect = CrystalRect::new(582.0, 430.0, 348.0, 32.0);

#[derive(Component)]
pub struct PcLoginButton {
    primary: bool,
    enabled: bool,
}

fn node(rect: CrystalRect) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(rect.left),
        top: Val::Px(rect.top),
        width: Val::Px(rect.width),
        height: Val::Px(rect.height),
        ..default()
    }
}
fn label(
    parent: &mut ChildSpawnerCommands,
    text: impl Into<String>,
    rect: CrystalRect,
    size: f32,
    color: Color,
) {
    parent.spawn((
        node(rect),
        Text::new(text),
        login_font(size),
        TextColor(color),
    ));
}
fn button(
    parent: &mut ChildSpawnerCommands,
    text: String,
    rect: CrystalRect,
    action: CrystalLoginAction,
    primary: bool,
    enabled: bool,
) {
    let mut entity = parent.spawn((
        Button,
        PcLoginButton { primary, enabled },
        Node {
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border: UiRect::all(Val::Px(1.0)),
            padding: UiRect::horizontal(Val::Px(10.0)),
            ..node(rect)
        },
        BackgroundColor(if primary { GOLD } else { PANEL }),
        BorderColor::all(EDGE),
    ));
    // Disabled controls carry no action and cannot submit by pointer.
    if enabled {
        entity.insert(action);
    }
    entity.with_children(|child| {
        child.spawn((
            Text::new(text),
            login_font(if primary { 16.0 } else { 13.0 }),
            TextColor(if primary {
                INK
            } else if enabled {
                TEXT
            } else {
                MUTED
            }),
            TextLayout::justify(Justify::Center),
        ));
    });
}
fn field(
    parent: &mut ChildSpawnerCommands,
    text: &str,
    rect: CrystalRect,
    action: CrystalLoginAction,
    focused: bool,
) {
    parent
        .spawn((
            Button,
            action,
            Node {
                padding: UiRect::horizontal(Val::Px(10.0)),
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                overflow: Overflow::clip(),
                ..node(rect)
            },
            BackgroundColor(INK),
            BorderColor::all(if focused { GOLD } else { EDGE }),
        ))
        .with_children(|child| {
            child.spawn((Text::new(text), login_font(14.0), TextColor(TEXT)));
            if focused {
                child.spawn((
                    CrystalLoginCaret::default(),
                    Text::new("|"),
                    login_font(14.0),
                    TextColor(GOLD),
                    Visibility::Visible,
                ));
            }
        });
}

pub fn spawn_pc_login_screen(
    parent: &mut ChildSpawnerCommands,
    model: &NativeShellModel,
    masked: &str,
    locale: NativeUiLocale,
) {
    parent.spawn((
        node(CrystalRect::new(0.0, 0.0, 1024.0, 768.0)),
        BackgroundColor(INK),
    ));
    parent.spawn((
        node(CrystalRect::new(64.0, 160.0, 36.0, 3.0)),
        BackgroundColor(GOLD),
    ));
    label(
        parent,
        "NUMERON",
        CrystalRect::new(64.0, 194.0, 458.0, 74.0),
        52.0,
        TEXT,
    );
    label(
        parent,
        "WINDOWS  /  PC CLIENT",
        CrystalRect::new(68.0, 280.0, 380.0, 28.0),
        14.0,
        GOLD,
    );
    parent.spawn((
        node(CrystalRect::new(68.0, 330.0, 394.0, 1.0)),
        BackgroundColor(EDGE),
    ));
    label(
        parent,
        "Dubhe Wallet",
        CrystalRect::new(68.0, 359.0, 380.0, 34.0),
        24.0,
        TEXT,
    );
    label(
        parent,
        locale.wallet_instructions(),
        CrystalRect::new(68.0, 413.0, 360.0, 110.0),
        17.0,
        MUTED,
    );
    parent.spawn((
        Node {
            border: UiRect::all(Val::Px(1.0)),
            ..node(CrystalRect::new(550.0, 120.0, 412.0, 552.0))
        },
        BackgroundColor(PANEL),
        BorderColor::all(EDGE),
    ));
    label(
        parent,
        locale.login_caption(Caption::Title),
        CrystalRect::new(582.0, 152.0, 348.0, 36.0),
        25.0,
        TEXT,
    );
    button(
        parent,
        locale.wallet_entry(),
        WALLET,
        CrystalLoginAction::DubheWallet,
        true,
        true,
    );
    label(
        parent,
        locale.wallet_instructions(),
        CrystalRect::new(582.0, 275.0, 348.0, 58.0),
        13.0,
        MUTED,
    );
    parent.spawn((
        node(CrystalRect::new(582.0, 333.0, 348.0, 1.0)),
        BackgroundColor(EDGE),
    ));
    label(
        parent,
        locale.login_caption(Caption::Account),
        CrystalRect::new(582.0, 343.0, 348.0, 20.0),
        13.0,
        MUTED,
    );
    field(
        parent,
        &model.login.account,
        ACCOUNT,
        CrystalLoginAction::FocusAccount,
        matches!(model.login.focus, LoginFocus::Account),
    );
    label(
        parent,
        locale.login_caption(Caption::Password),
        CrystalRect::new(582.0, 408.0, 348.0, 20.0),
        13.0,
        MUTED,
    );
    field(
        parent,
        masked,
        PASSWORD,
        CrystalLoginAction::FocusPassword,
        matches!(model.login.focus, LoginFocus::Password),
    );
    button(
        parent,
        locale.login_caption(Caption::Login),
        CrystalRect::new(582.0, 483.0, 348.0, 38.0),
        CrystalLoginAction::Login,
        false,
        model.login.is_ready(),
    );
    for (caption, action, x, y) in [
        (
            Caption::NewAccount,
            CrystalLoginAction::RegisterAccount,
            582.0,
            535.0,
        ),
        (
            Caption::ChangePassword,
            CrystalLoginAction::ChangePassword,
            762.0,
            535.0,
        ),
        (Caption::SafeKey, CrystalLoginAction::SafeKey, 582.0, 580.0),
        (Caption::Cancel, CrystalLoginAction::Cancel, 762.0, 580.0),
    ] {
        button(
            parent,
            locale.login_caption(caption),
            CrystalRect::new(x, y, 168.0, 34.0),
            action,
            false,
            true,
        );
    }
    if let Some(notice) = &model.notice {
        label(
            parent,
            crate::native_i18n::tr(&notice.message),
            CrystalRect::new(68.0, 550.0, 390.0, 130.0),
            15.0,
            if matches!(notice.kind, ShellNoticeKind::Error) {
                Color::srgb(0.90, 0.48, 0.42)
            } else {
                GOLD
            },
        );
    }
}

pub fn sync_pc_login_buttons(
    mut buttons: Query<(&Interaction, &PcLoginButton, &mut BackgroundColor), Changed<Interaction>>,
) {
    for (interaction, button, mut color) in &mut buttons {
        color.0 = match (button.enabled, interaction, button.primary) {
            (false, _, _) => PANEL,
            (true, Interaction::Pressed, _) => Color::srgb(0.48, 0.40, 0.25),
            (true, Interaction::Hovered, true) => Color::srgb(0.87, 0.76, 0.53),
            (true, Interaction::Hovered, false) => Color::srgb(0.17, 0.18, 0.18),
            (_, _, true) => GOLD,
            _ => PANEL,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pc_wallet_primary_and_legacy_fields_are_separate() {
        assert!(WALLET.top + WALLET.height < ACCOUNT.top);
        assert!(ACCOUNT.top + ACCOUNT.height < PASSWORD.top);
        for rect in [WALLET, ACCOUNT, PASSWORD] {
            assert!(rect.left >= 0.0 && rect.left + rect.width <= 1024.0);
            assert!(rect.top >= 0.0 && rect.top + rect.height <= 768.0);
        }
    }
    fn scene(ready: bool) -> App {
        let mut app = App::new();
        app.add_systems(Startup, move |mut commands: Commands| {
            let mut model = NativeShellModel::default();
            if ready {
                model.login.account = "test-account".into();
                model.login.password = "not-a-real-password".into();
            }
            commands.spawn(Node::default()).with_children(|parent| {
                spawn_pc_login_screen(parent, &model, &"*".repeat(model.login.password.len()), NativeUiLocale::default());
            });
        });
        app.update();
        app
    }
    #[test]
    fn pc_wallet_first_controls_keep_real_actions_and_mask_password() {
        let mut app = scene(true);
        let actions = app.world_mut().query::<&CrystalLoginAction>().iter(app.world()).copied().collect::<Vec<_>>();
        for expected in [CrystalLoginAction::DubheWallet, CrystalLoginAction::FocusAccount,
            CrystalLoginAction::FocusPassword, CrystalLoginAction::Login, CrystalLoginAction::RegisterAccount,
            CrystalLoginAction::ChangePassword, CrystalLoginAction::SafeKey, CrystalLoginAction::Cancel] {
            assert_eq!(actions.iter().filter(|action| **action == expected).count(), 1);
        }
        assert!(!app.world_mut().query::<&Text>().iter(app.world()).any(|text| text.0.contains("not-a-real-password")));
    }
    #[test]
    fn pc_empty_account_cannot_submit_by_pointer_but_wallet_stays_available() {
        let mut app = scene(false);
        let actions = app.world_mut().query::<&CrystalLoginAction>().iter(app.world()).copied().collect::<Vec<_>>();
        assert!(!actions.contains(&CrystalLoginAction::Login));
        assert!(actions.contains(&CrystalLoginAction::DubheWallet));
    }
}
