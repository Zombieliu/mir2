#![forbid(unsafe_code)]

use bevy::app::AppExit;
use bevy::ui::{
    widget::NodeImageMode, AlignItems, Display, FlexDirection, JustifyContent, Node, PositionType,
    UiRect, Val,
};
use bevy::{input::keyboard::KeyboardInput, prelude::*};

use crate::crystal_ui::assets::{safe_key_assets, CrystalButtonAssetSet};
use crate::crystal_ui::login::{blink_login_caret, spawn_login_screen, CrystalLoginAction};
use crate::crystal_ui::select::{
    animate_character_previews, spawn_character_select_screen, CrystalSelectAction,
};
use crate::crystal_ui::spec::{self, CrystalButtonSpec};
use crate::crystal_ui::widget::{
    sync_crystal_image_buttons, CrystalImageButton, CrystalImageButtonSprite,
};
use crate::native_shell::{
    ChangePasswordFocus, CharacterCreateFocus, LoginFocus, NativeShellModel, NativeShellScreen,
    NativeUiIntent, NativeUiIntentQueue,
};

const ROOT_BG: Color = Color::srgba(0.06, 0.05, 0.03, 0.82);
const PANEL_BG: Color = Color::srgba(0.16, 0.11, 0.07, 0.96);
const GOLD: Color = Color::srgb(0.94, 0.78, 0.28);
const CREAM: Color = Color::srgb(0.95, 0.92, 0.82);
const BUTTON_BG: Color = Color::srgba(0.29, 0.19, 0.11, 1.0);
const BUTTON_HIGHLIGHT: Color = Color::srgb(0.82, 0.63, 0.23);
const BUTTON_DISABLED: Color = Color::srgba(0.19, 0.14, 0.10, 0.50);

const MAX_ACCOUNT: usize = 24;
const MAX_PASSWORD: usize = 32;
const MAX_CHANGE_ACCOUNT: usize = 15;
const MAX_CHANGE_PASSWORD: usize = 15;
const MAX_NAME: usize = 18;
const CLASSES: [&str; 3] = ["Warrior", "Wizard", "Taoist"];
const GENDERS: [&str; 2] = ["Male", "Female"];

pub fn password_mask(password: &str) -> String {
    "*".repeat(password.chars().count())
}

pub fn is_printable_char(c: char) -> bool {
    !c.is_control()
}

pub fn rotate_choice(current: &str, options: &[&str], reverse: bool) -> String {
    if options.is_empty() {
        return current.to_owned();
    }
    let i = options
        .iter()
        .position(|item| item.eq_ignore_ascii_case(current))
        .unwrap_or(0);
    let next = if reverse {
        if i == 0 {
            options.len() - 1
        } else {
            i - 1
        }
    } else {
        (i + 1) % options.len()
    };
    options[next].to_owned()
}

pub fn cycle_login_focus(current: LoginFocus, reverse: bool) -> LoginFocus {
    match (current, reverse) {
        (LoginFocus::Account, false) => LoginFocus::Password,
        (LoginFocus::Password, false) => LoginFocus::LoginButton,
        (LoginFocus::LoginButton, false) => LoginFocus::NewAccountButton,
        (LoginFocus::NewAccountButton, false) => LoginFocus::Account,
        (LoginFocus::Account, true) => LoginFocus::NewAccountButton,
        (LoginFocus::Password, true) => LoginFocus::Account,
        (LoginFocus::LoginButton, true) => LoginFocus::Password,
        (LoginFocus::NewAccountButton, true) => LoginFocus::LoginButton,
    }
}

pub fn cycle_create_focus(current: CharacterCreateFocus, reverse: bool) -> CharacterCreateFocus {
    match (current, reverse) {
        (CharacterCreateFocus::Name, false) => CharacterCreateFocus::Class,
        (CharacterCreateFocus::Class, false) => CharacterCreateFocus::Gender,
        (CharacterCreateFocus::Gender, false) => CharacterCreateFocus::CreateButton,
        (CharacterCreateFocus::CreateButton, false) => CharacterCreateFocus::CancelButton,
        (CharacterCreateFocus::CancelButton, false) => CharacterCreateFocus::Name,
        (CharacterCreateFocus::Name, true) => CharacterCreateFocus::CancelButton,
        (CharacterCreateFocus::Class, true) => CharacterCreateFocus::Name,
        (CharacterCreateFocus::Gender, true) => CharacterCreateFocus::Class,
        (CharacterCreateFocus::CreateButton, true) => CharacterCreateFocus::Gender,
        (CharacterCreateFocus::CancelButton, true) => CharacterCreateFocus::CreateButton,
    }
}

fn is_gateway_intent(intent: &NativeUiIntent) -> bool {
    matches!(
        intent,
        NativeUiIntent::Login
            | NativeUiIntent::RegisterAccount
            | NativeUiIntent::CreateCharacter { .. }
            | NativeUiIntent::ConfirmDeleteCharacter
            | NativeUiIntent::SubmitChangePassword { .. }
            | NativeUiIntent::SafeKeyEnter
            | NativeUiIntent::StartGame
            | NativeUiIntent::Retry
            | NativeUiIntent::Logout,
    )
}

fn apply_and_queue(
    model: &mut NativeShellModel,
    queue: &mut NativeUiIntentQueue,
    intent: NativeUiIntent,
) -> bool {
    let ok = model.apply_ui_intent(intent.clone());
    if ok && is_gateway_intent(&intent) {
        queue.push(intent);
    }
    ok
}

fn change_password_submit_intent(model: &NativeShellModel) -> NativeUiIntent {
    NativeUiIntent::SubmitChangePassword {
        account_id: model.change_password.account_id.clone(),
        old_password: model.change_password.old_password.clone(),
        new_password: model.change_password.new_password.clone(),
        confirm_password: model.change_password.confirm_password.clone(),
    }
}

pub fn cycle_change_password_focus(
    current: ChangePasswordFocus,
    reverse: bool,
) -> ChangePasswordFocus {
    match (current, reverse) {
        (ChangePasswordFocus::AccountId, false) => ChangePasswordFocus::OldPassword,
        (ChangePasswordFocus::OldPassword, false) => ChangePasswordFocus::NewPassword,
        (ChangePasswordFocus::NewPassword, false) => ChangePasswordFocus::ConfirmPassword,
        (ChangePasswordFocus::ConfirmPassword, false) => ChangePasswordFocus::SubmitButton,
        (ChangePasswordFocus::SubmitButton, false) => ChangePasswordFocus::CancelButton,
        (ChangePasswordFocus::CancelButton, false) => ChangePasswordFocus::AccountId,
        (ChangePasswordFocus::AccountId, true) => ChangePasswordFocus::CancelButton,
        (ChangePasswordFocus::OldPassword, true) => ChangePasswordFocus::AccountId,
        (ChangePasswordFocus::NewPassword, true) => ChangePasswordFocus::OldPassword,
        (ChangePasswordFocus::ConfirmPassword, true) => ChangePasswordFocus::NewPassword,
        (ChangePasswordFocus::SubmitButton, true) => ChangePasswordFocus::ConfirmPassword,
        (ChangePasswordFocus::CancelButton, true) => ChangePasswordFocus::SubmitButton,
    }
}

#[derive(Component)]
struct NativeShellRoot;

#[derive(Component)]
struct NativeShellContent;

#[derive(Component)]
enum NativeShellButton {
    CancelCreate,
    SubmitCreate,
    Retry,
    CycleClass,
    CycleGender,
    ConfirmDelete,
    CancelDelete,
    SubmitChangePassword,
    CancelChangePassword,
    CloseSafeKey,
    SafeKeyFocusAccount,
    SafeKeyFocusPassword,
    SafeKeyPress(char),
    SafeKeyDelete,
    SafeKeyRandom,
    SafeKeyEnter,
}

pub struct Mir2NativeShellUiPlugin;

impl Plugin for Mir2NativeShellUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NativeUiIntentQueue>()
            .add_systems(Startup, spawn_shell_ui)
            .add_systems(
                Update,
                (
                    update_root_visibility,
                    shell_keyboard_input,
                    shell_pointer_input,
                    render_shell_ui,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    sync_crystal_image_buttons,
                    blink_login_caret,
                    animate_character_previews,
                )
                    .after(render_shell_ui),
            );
    }
}

fn spawn_shell_ui(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands
        .spawn((
            NativeShellRoot,
            Node {
                width: Val::Px(1024.0),
                height: Val::Px(768.0),
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                display: Display::None,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(ROOT_BG),
            GlobalZIndex(1_000),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                ImageNode {
                    image: asset_server.load("original-ui/ChrSel/0.png"),
                    image_mode: NodeImageMode::Stretch,
                    ..default()
                },
            ));
            root.spawn((
                NativeShellContent,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
            ));
        });
}

fn update_root_visibility(
    shell: Option<Res<NativeShellModel>>,
    mut roots: Query<&mut Node, With<NativeShellRoot>>,
) {
    let Ok(mut root) = roots.single_mut() else {
        return;
    };

    let Some(shell) = shell else {
        root.display = Display::None;
        return;
    };

    root.display = if shell.screen == NativeShellScreen::InGame {
        Display::None
    } else {
        Display::Flex
    };
}

fn shell_pointer_input(
    native_interactions: Query<
        (&Interaction, &NativeShellButton),
        (Changed<Interaction>, With<Button>),
    >,
    login_interactions: Query<
        (&Interaction, &CrystalLoginAction),
        (Changed<Interaction>, With<Button>),
    >,
    select_interactions: Query<
        (&Interaction, &CrystalSelectAction),
        (Changed<Interaction>, With<Button>),
    >,
    shell: Option<ResMut<NativeShellModel>>,
    queue: Option<ResMut<NativeUiIntentQueue>>,
    mut app_exit: MessageWriter<AppExit>,
) {
    let (Some(mut shell), Some(mut queue)) = (shell, queue) else {
        return;
    };

    if shell.screen == NativeShellScreen::InGame {
        return;
    }

    for (interaction, action) in login_interactions.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let login_surface = matches!(
            shell.screen,
            NativeShellScreen::Login | NativeShellScreen::SafeKey
        );
        if !login_surface {
            continue;
        }
        if shell.screen == NativeShellScreen::SafeKey
            && !matches!(
                action,
                CrystalLoginAction::FocusAccount | CrystalLoginAction::FocusPassword
            )
        {
            continue;
        }

        match action {
            CrystalLoginAction::FocusAccount => {
                shell.login.focus = LoginFocus::Account;
            }
            CrystalLoginAction::FocusPassword => {
                shell.login.focus = LoginFocus::Password;
            }
            CrystalLoginAction::Login => {
                apply_and_queue(&mut shell, &mut queue, NativeUiIntent::Login);
            }
            CrystalLoginAction::RegisterAccount => {
                apply_and_queue(&mut shell, &mut queue, NativeUiIntent::RegisterAccount);
            }
            CrystalLoginAction::ChangePassword => {
                let _ = shell.apply_ui_intent(NativeUiIntent::OpenChangePassword);
            }
            CrystalLoginAction::SafeKey => {
                let _ = shell.apply_ui_intent(NativeUiIntent::OpenSafeKey);
            }
            CrystalLoginAction::Cancel => {
                app_exit.write(AppExit::Success);
            }
        }
    }

    for (interaction, action) in select_interactions.iter() {
        if *interaction != Interaction::Pressed
            || shell.screen != NativeShellScreen::CharacterSelect
        {
            continue;
        }

        match action {
            CrystalSelectAction::SelectCharacter(index) => {
                let _ = shell.apply_ui_intent(NativeUiIntent::SelectCharacter {
                    character_index: *index,
                });
            }
            CrystalSelectAction::Start => {
                apply_and_queue(&mut shell, &mut queue, NativeUiIntent::StartGame);
            }
            CrystalSelectAction::NewCharacter => {
                let _ = shell.apply_ui_intent(NativeUiIntent::OpenCharacterCreate);
            }
            CrystalSelectAction::DeleteCharacter => {
                if let Some(character_index) = shell.selected_character_index {
                    apply_and_queue(
                        &mut shell,
                        &mut queue,
                        NativeUiIntent::DeleteCharacter { character_index },
                    );
                }
            }
            // Crystal's SelectScene credits handler is intentionally empty.
            CrystalSelectAction::Credits => {}
            CrystalSelectAction::Exit => {
                app_exit.write(AppExit::Success);
            }
        }
    }

    for (interaction, action) in native_interactions.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }

        match action {
            NativeShellButton::CancelCreate => {
                let _ = shell.apply_ui_intent(NativeUiIntent::CancelCharacterCreate);
            }
            NativeShellButton::SubmitCreate => {
                let intent = NativeUiIntent::CreateCharacter {
                    name: shell.character_create.name.clone(),
                    class_name: shell.character_create.class_name.clone(),
                    gender_name: shell.character_create.gender_name.clone(),
                };
                apply_and_queue(&mut shell, &mut queue, intent);
            }
            NativeShellButton::Retry => {
                apply_and_queue(&mut shell, &mut queue, NativeUiIntent::Retry);
            }
            NativeShellButton::CycleClass => {
                shell.character_create.class_name =
                    rotate_choice(&shell.character_create.class_name, &CLASSES, false);
            }
            NativeShellButton::CycleGender => {
                shell.character_create.gender_name =
                    rotate_choice(&shell.character_create.gender_name, &GENDERS, false);
            }
            NativeShellButton::ConfirmDelete => {
                apply_and_queue(
                    &mut shell,
                    &mut queue,
                    NativeUiIntent::ConfirmDeleteCharacter,
                );
            }
            NativeShellButton::CancelDelete => {
                let _ = shell.apply_ui_intent(NativeUiIntent::CancelDeleteCharacter);
            }
            NativeShellButton::SubmitChangePassword => {
                let intent = change_password_submit_intent(&shell);
                apply_and_queue(&mut shell, &mut queue, intent);
            }
            NativeShellButton::CancelChangePassword => {
                let _ = shell.apply_ui_intent(NativeUiIntent::CancelChangePassword);
            }
            NativeShellButton::CloseSafeKey => {
                let _ = shell.apply_ui_intent(NativeUiIntent::CloseSafeKey);
            }
            NativeShellButton::SafeKeyFocusAccount => {
                let _ = shell.apply_ui_intent(NativeUiIntent::SafeKeyFocusAccount);
            }
            NativeShellButton::SafeKeyFocusPassword => {
                let _ = shell.apply_ui_intent(NativeUiIntent::SafeKeyFocusPassword);
            }
            NativeShellButton::SafeKeyPress(key) => {
                let _ = shell.apply_ui_intent(NativeUiIntent::SafeKeyPress { key: *key });
            }
            NativeShellButton::SafeKeyDelete => {
                let _ = shell.apply_ui_intent(NativeUiIntent::SafeKeyDelete);
            }
            NativeShellButton::SafeKeyRandom => {
                let _ = shell.apply_ui_intent(NativeUiIntent::SafeKeyRandom);
            }
            NativeShellButton::SafeKeyEnter => {
                apply_and_queue(&mut shell, &mut queue, NativeUiIntent::SafeKeyEnter);
            }
        }
    }
}

fn shell_keyboard_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut keyboard_inputs: MessageReader<KeyboardInput>,
    shell: Option<ResMut<NativeShellModel>>,
    queue: Option<ResMut<NativeUiIntentQueue>>,
) {
    let (Some(mut shell), Some(mut queue)) = (shell, queue) else {
        return;
    };

    if shell.screen == NativeShellScreen::InGame {
        return;
    }

    let typed_text = keyboard_inputs
        .read()
        .filter(|event| event.state.is_pressed())
        .filter_map(|event| event.text.as_deref())
        .collect::<String>();
    let shifted = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);

    match shell.screen {
        NativeShellScreen::Login => {
            if keys.just_pressed(KeyCode::Tab) {
                shell.login.focus = cycle_login_focus(shell.login.focus, shifted);
                return;
            }

            if keys.just_pressed(KeyCode::Escape) {
                shell.login.focus = LoginFocus::Account;
                return;
            }

            if keys.just_pressed(KeyCode::Enter) {
                match shell.login.focus {
                    LoginFocus::Account => shell.login.focus = LoginFocus::Password,
                    LoginFocus::Password => shell.login.focus = LoginFocus::LoginButton,
                    LoginFocus::LoginButton => {
                        apply_and_queue(&mut shell, &mut queue, NativeUiIntent::Login);
                    }
                    LoginFocus::NewAccountButton => {
                        apply_and_queue(&mut shell, &mut queue, NativeUiIntent::RegisterAccount);
                    }
                }
                return;
            }

            if keys.just_pressed(KeyCode::Backspace) {
                match shell.login.focus {
                    LoginFocus::Account => {
                        shell.login.account.pop();
                    }
                    LoginFocus::Password => {
                        shell.login.password.pop();
                    }
                    LoginFocus::LoginButton | LoginFocus::NewAccountButton => {}
                }
            }

            for c in typed_text.chars() {
                match shell.login.focus {
                    LoginFocus::Account => {
                        append_editable_field(&mut shell.login.account, c, MAX_ACCOUNT)
                    }
                    LoginFocus::Password => {
                        append_editable_field(&mut shell.login.password, c, MAX_PASSWORD)
                    }
                    LoginFocus::LoginButton | LoginFocus::NewAccountButton => {}
                }
            }
        }

        NativeShellScreen::CharacterCreate => {
            if keys.just_pressed(KeyCode::Tab) {
                shell.character_create.focus =
                    cycle_create_focus(shell.character_create.focus, shifted);
                return;
            }

            if keys.just_pressed(KeyCode::Escape) {
                let _ = shell.apply_ui_intent(NativeUiIntent::CancelCharacterCreate);
                return;
            }

            if keys.just_pressed(KeyCode::Backspace) {
                if matches!(shell.character_create.focus, CharacterCreateFocus::Name) {
                    shell.character_create.name.pop();
                }
            }

            if keys.just_pressed(KeyCode::Enter) {
                match shell.character_create.focus {
                    CharacterCreateFocus::Name => {
                        shell.character_create.focus = CharacterCreateFocus::Class
                    }
                    CharacterCreateFocus::Class => {
                        shell.character_create.class_name =
                            rotate_choice(&shell.character_create.class_name, &CLASSES, false)
                    }
                    CharacterCreateFocus::Gender => {
                        shell.character_create.gender_name =
                            rotate_choice(&shell.character_create.gender_name, &GENDERS, false)
                    }
                    CharacterCreateFocus::CreateButton => {
                        let intent = NativeUiIntent::CreateCharacter {
                            name: shell.character_create.name.clone(),
                            class_name: shell.character_create.class_name.clone(),
                            gender_name: shell.character_create.gender_name.clone(),
                        };
                        apply_and_queue(&mut shell, &mut queue, intent);
                    }
                    CharacterCreateFocus::CancelButton => {
                        let _ = shell.apply_ui_intent(NativeUiIntent::CancelCharacterCreate);
                    }
                }
                return;
            }

            if matches!(shell.character_create.focus, CharacterCreateFocus::Name) {
                for c in typed_text.chars() {
                    append_editable_field(&mut shell.character_create.name, c, MAX_NAME);
                }
            }
        }

        NativeShellScreen::CharacterSelect => {
            if keys.just_pressed(KeyCode::Escape) {
                apply_and_queue(&mut shell, &mut queue, NativeUiIntent::Logout);
            }
            if keys.just_pressed(KeyCode::Enter) {
                apply_and_queue(&mut shell, &mut queue, NativeUiIntent::StartGame);
            }
        }

        NativeShellScreen::ChangePassword => {
            if keys.just_pressed(KeyCode::Tab) {
                let shifted = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
                shell.change_password.focus =
                    cycle_change_password_focus(shell.change_password.focus, shifted);
                return;
            }
            if keys.just_pressed(KeyCode::Escape) {
                let _ = shell.apply_ui_intent(NativeUiIntent::CancelChangePassword);
                return;
            }
            if keys.just_pressed(KeyCode::Enter) {
                match shell.change_password.focus {
                    ChangePasswordFocus::SubmitButton => {
                        let intent = change_password_submit_intent(&shell);
                        apply_and_queue(&mut shell, &mut queue, intent);
                    }
                    ChangePasswordFocus::CancelButton => {
                        let _ = shell.apply_ui_intent(NativeUiIntent::CancelChangePassword);
                    }
                    _ => {}
                }
                return;
            }
            if keys.just_pressed(KeyCode::Backspace) {
                match shell.change_password.focus {
                    ChangePasswordFocus::AccountId => {
                        shell.change_password.account_id.pop();
                    }
                    ChangePasswordFocus::OldPassword => {
                        shell.change_password.old_password.pop();
                    }
                    ChangePasswordFocus::NewPassword => {
                        shell.change_password.new_password.pop();
                    }
                    ChangePasswordFocus::ConfirmPassword => {
                        shell.change_password.confirm_password.pop();
                    }
                    _ => {}
                }
            }
            for c in typed_text.chars() {
                match shell.change_password.focus {
                    ChangePasswordFocus::AccountId => append_editable_field(
                        &mut shell.change_password.account_id,
                        c,
                        MAX_CHANGE_ACCOUNT,
                    ),
                    ChangePasswordFocus::OldPassword => append_editable_field(
                        &mut shell.change_password.old_password,
                        c,
                        MAX_CHANGE_PASSWORD,
                    ),
                    ChangePasswordFocus::NewPassword => append_editable_field(
                        &mut shell.change_password.new_password,
                        c,
                        MAX_CHANGE_PASSWORD,
                    ),
                    ChangePasswordFocus::ConfirmPassword => append_editable_field(
                        &mut shell.change_password.confirm_password,
                        c,
                        MAX_CHANGE_PASSWORD,
                    ),
                    _ => {}
                }
            }
        }
        NativeShellScreen::SafeKey => {
            if keys.just_pressed(KeyCode::Escape) {
                let _ = shell.apply_ui_intent(NativeUiIntent::CloseSafeKey);
                return;
            }
            if keys.just_pressed(KeyCode::Tab) {
                shell.login.focus = match shell.login.focus {
                    LoginFocus::Account => LoginFocus::Password,
                    _ => LoginFocus::Account,
                };
                return;
            }
            if keys.just_pressed(KeyCode::Backspace) {
                let _ = shell.apply_ui_intent(NativeUiIntent::SafeKeyDelete);
                return;
            }
            if keys.just_pressed(KeyCode::Enter) {
                apply_and_queue(&mut shell, &mut queue, NativeUiIntent::SafeKeyEnter);
                return;
            }
        }
        NativeShellScreen::DeleteConfirm { .. } => {
            if keys.just_pressed(KeyCode::Escape) {
                let _ = shell.apply_ui_intent(NativeUiIntent::CancelDeleteCharacter);
                return;
            }
            if keys.just_pressed(KeyCode::Enter) {
                apply_and_queue(
                    &mut shell,
                    &mut queue,
                    NativeUiIntent::ConfirmDeleteCharacter,
                );
                return;
            }
        }
        NativeShellScreen::ConnectionLost => {
            if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Escape) {
                apply_and_queue(&mut shell, &mut queue, NativeUiIntent::Retry);
            }
        }

        _ => {}
    }
}

fn append_editable_field(text: &mut String, c: char, max: usize) {
    if !is_printable_char(c) {
        return;
    }
    if text.chars().count() >= max {
        return;
    }
    text.push(c);
}

fn render_shell_ui(
    mut commands: Commands,
    model: Option<Res<NativeShellModel>>,
    asset_server: Res<AssetServer>,
    content_nodes: Query<Entity, With<NativeShellContent>>,
    mut last_rendered_model: Local<Option<NativeShellModel>>,
) {
    let Some(model) = model else {
        return;
    };

    if model.screen == NativeShellScreen::InGame {
        return;
    }

    if last_rendered_model.as_ref() == Some(model.as_ref()) {
        return;
    }
    *last_rendered_model = Some(model.clone());

    let Ok(content) = content_nodes.single() else {
        return;
    };

    commands.entity(content).despawn_children();

    commands
        .entity(content)
        .with_children(|screen| match model.screen {
            NativeShellScreen::Connecting => {
                with_generic_panel(screen, |panel| {
                    info_block(panel, "Connecting", "Connecting to gateway...");
                });
            }
            NativeShellScreen::Authenticating => {
                with_generic_panel(screen, |panel| {
                    info_block(panel, "Authenticating", "Authenticating account...");
                });
            }
            NativeShellScreen::StartingGame => {
                with_generic_panel(screen, |panel| {
                    info_block(panel, "Starting", "Entering game world...");
                });
            }
            NativeShellScreen::Login => {
                spawn_login_screen(
                    screen,
                    &asset_server,
                    &model,
                    &password_mask(&model.login.password),
                );
            }
            NativeShellScreen::CharacterSelect => {
                spawn_character_select_screen(screen, &asset_server, &model);
            }
            NativeShellScreen::CharacterCreate => with_generic_panel(screen, |panel| {
                render_character_create(panel, &model);
            }),
            NativeShellScreen::ChangePassword => with_generic_panel(screen, |panel| {
                render_change_password(panel, &model);
            }),
            NativeShellScreen::SafeKey => {
                spawn_login_screen(
                    screen,
                    &asset_server,
                    &model,
                    &password_mask(&model.login.password),
                );
                render_safe_key(screen, &asset_server, &model);
            }
            NativeShellScreen::DeleteConfirm { index } => with_generic_panel(screen, |panel| {
                render_delete_confirm(panel, &model, index);
            }),
            NativeShellScreen::ConnectionLost => with_generic_panel(screen, |panel| {
                render_connection_lost(panel, &model);
            }),
            NativeShellScreen::InGame => {}
        });
}

fn with_generic_panel(
    parent: &mut ChildSpawnerCommands,
    render: impl FnOnce(&mut ChildSpawnerCommands),
) {
    parent
        .spawn((
            Node {
                width: Val::Px(620.0),
                min_height: Val::Px(360.0),
                position_type: PositionType::Absolute,
                left: Val::Px((1024.0 - 620.0) * 0.5),
                top: Val::Px((768.0 - 360.0) * 0.5),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::FlexStart,
                align_items: AlignItems::FlexStart,
                padding: UiRect::all(Val::Px(20.0)),
                row_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(PANEL_BG),
        ))
        .with_children(render);
}

fn info_block(parent: &mut ChildSpawnerCommands, title: &str, detail: &str) {
    title_line(parent, title);
    text_line(parent, detail);
}

fn render_character_create(parent: &mut ChildSpawnerCommands, model: &NativeShellModel) {
    title_line(parent, "Create Character");
    input_line(
        parent,
        "Name",
        &model.character_create.name,
        model.character_create.focus == CharacterCreateFocus::Name,
    );
    input_line(
        parent,
        "Class",
        &model.character_create.class_name,
        model.character_create.focus == CharacterCreateFocus::Class,
    );
    input_line(
        parent,
        "Gender",
        &model.character_create.gender_name,
        model.character_create.focus == CharacterCreateFocus::Gender,
    );

    parent
        .spawn((Node {
            width: Val::Percent(100.0),
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(10.0),
            ..default()
        },))
        .with_children(|row| {
            action_button(row, "Class", NativeShellButton::CycleClass, true, false);
            action_button(row, "Gender", NativeShellButton::CycleGender, true, false);
        });

    if let Some(notice) = &model.notice {
        text_line(parent, &notice.message);
    }

    parent
        .spawn((Node {
            width: Val::Percent(100.0),
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(10.0),
            ..default()
        },))
        .with_children(|row| {
            action_button(
                row,
                "Create",
                NativeShellButton::SubmitCreate,
                true,
                model.character_create.focus == CharacterCreateFocus::CreateButton,
            );
            action_button(
                row,
                "Cancel",
                NativeShellButton::CancelCreate,
                true,
                model.character_create.focus == CharacterCreateFocus::CancelButton,
            );
        });
}

fn render_change_password(parent: &mut ChildSpawnerCommands, model: &NativeShellModel) {
    title_line(parent, "Change Password");
    input_line(
        parent,
        "Account ID",
        &model.change_password.account_id,
        model.change_password.focus == ChangePasswordFocus::AccountId,
    );
    input_line(
        parent,
        "Current Password",
        &password_mask(&model.change_password.old_password),
        model.change_password.focus == ChangePasswordFocus::OldPassword,
    );
    input_line(
        parent,
        "New Password",
        &password_mask(&model.change_password.new_password),
        model.change_password.focus == ChangePasswordFocus::NewPassword,
    );
    input_line(
        parent,
        "Confirm Password",
        &password_mask(&model.change_password.confirm_password),
        model.change_password.focus == ChangePasswordFocus::ConfirmPassword,
    );
    if let Some(notice) = &model.notice {
        text_line(parent, &notice.message);
    }
    parent
        .spawn((Node {
            width: Val::Percent(100.0),
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(10.0),
            ..default()
        },))
        .with_children(|row| {
            action_button(
                row,
                "Submit",
                NativeShellButton::SubmitChangePassword,
                !model.change_password_request_in_flight,
                model.change_password.focus == ChangePasswordFocus::SubmitButton,
            );
            action_button(
                row,
                "Cancel",
                NativeShellButton::CancelChangePassword,
                !model.change_password_request_in_flight,
                model.change_password.focus == ChangePasswordFocus::CancelButton,
            );
        });
}

fn render_safe_key(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    model: &NativeShellModel,
) {
    let assets = safe_key_assets();

    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(spec::safe_key::PANEL.rect.left),
            top: Val::Px(spec::safe_key::PANEL.rect.top),
            width: Val::Px(spec::safe_key::PANEL.rect.width),
            height: Val::Px(spec::safe_key::PANEL.rect.height),
            ..default()
        },
        ImageNode {
            image: asset_server.load(assets.panel.clone()),
            image_mode: NodeImageMode::Stretch,
            ..default()
        },
    ));

    safe_key_image_button(
        parent,
        asset_server,
        spec::safe_key::ESC,
        assets.esc.clone(),
        "Esc",
        NativeShellButton::CloseSafeKey,
    );
    safe_key_image_button(
        parent,
        asset_server,
        spec::safe_key::DELETE,
        assets.delete.clone(),
        "Delete",
        NativeShellButton::SafeKeyDelete,
    );
    safe_key_image_button(
        parent,
        asset_server,
        spec::safe_key::ENTER,
        assets.enter.clone(),
        "Enter",
        NativeShellButton::SafeKeyEnter,
    );
    safe_key_image_button(
        parent,
        asset_server,
        spec::safe_key::RANDOM,
        assets.random.clone(),
        "Random",
        NativeShellButton::SafeKeyRandom,
    );

    for (index, key) in model
        .safe_key
        .keys
        .iter()
        .enumerate()
        .take(spec::safe_key::KEY_COUNT)
    {
        safe_key_image_button(
            parent,
            asset_server,
            spec::safe_key::key_spec(index),
            assets.key.clone(),
            &key.to_string(),
            NativeShellButton::SafeKeyPress(*key),
        );
    }
}

fn safe_key_image_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    spec: CrystalButtonSpec,
    assets: CrystalButtonAssetSet,
    label: &str,
    action: NativeShellButton,
) {
    let image = assets.normal.clone();
    parent
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(spec.rect.left),
                top: Val::Px(spec.rect.top),
                width: Val::Px(spec.rect.width),
                height: Val::Px(spec.rect.height),
                ..default()
            },
            Button,
            CrystalImageButton {
                assets,
                focused: false,
                enabled: true,
            },
            action,
        ))
        .with_children(|button| {
            button.spawn((
                CrystalImageButtonSprite,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Px(spec.image_width),
                    height: Val::Px(spec.image_height),
                    ..default()
                },
                ImageNode {
                    image: asset_server.load(image),
                    image_mode: NodeImageMode::Stretch,
                    ..default()
                },
            ));
            button.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    display: Display::Flex,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                Text::new(label.to_owned()),
                body_font(14.0),
                TextColor(CREAM),
            ));
        });
}

fn render_delete_confirm(parent: &mut ChildSpawnerCommands, model: &NativeShellModel, index: i32) {
    let name = model
        .characters
        .iter()
        .find(|c| c.index == index)
        .map(|c| c.name.as_str())
        .unwrap_or("Unknown");
    title_line(parent, "Delete Character");
    text_line(
        parent,
        &format!(
            "Are you sure you want to delete '{}' (index {})? This cannot be undone.",
            name, index
        ),
    );
    if let Some(notice) = &model.notice {
        text_line(parent, &notice.message);
    }
    parent
        .spawn((Node {
            width: Val::Percent(100.0),
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(10.0),
            ..default()
        },))
        .with_children(|row| {
            action_button(
                row,
                if model.delete_request_in_flight {
                    "Deleting..."
                } else {
                    "Confirm Delete"
                },
                NativeShellButton::ConfirmDelete,
                !model.delete_request_in_flight,
                false,
            );
            action_button(
                row,
                "Cancel",
                NativeShellButton::CancelDelete,
                !model.delete_request_in_flight,
                false,
            );
        });
}

fn render_connection_lost(parent: &mut ChildSpawnerCommands, model: &NativeShellModel) {
    title_line(parent, "Connection Lost");
    text_line(parent, "Press Enter or Escape to retry.");
    if let Some(notice) = &model.notice {
        text_line(parent, &notice.message);
    }
    action_button(parent, "Retry", NativeShellButton::Retry, true, false);
}

fn title_line(parent: &mut ChildSpawnerCommands, text: &str) {
    parent.spawn((
        Text::new(text.to_owned()),
        TextFont {
            font_size: FontSize::Px(28.0),
            ..default()
        },
        TextColor(GOLD),
    ));
}

fn text_line(parent: &mut ChildSpawnerCommands, text: &str) {
    parent.spawn((
        Text::new(text.to_owned()),
        body_font(16.0),
        TextColor(CREAM),
    ));
}

fn input_line(parent: &mut ChildSpawnerCommands, label: &str, value: &str, focused: bool) {
    parent
        .spawn((Node {
            width: Val::Percent(100.0),
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        },))
        .with_children(|row| {
            row.spawn((
                Text::new(format!("{label}")),
                body_font(14.0),
                TextColor(CREAM),
            ));
            row.spawn((
                Node {
                    width: Val::Px(360.0),
                    height: Val::Px(30.0),
                    display: Display::Flex,
                    align_items: AlignItems::Center,
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(if focused { BUTTON_HIGHLIGHT } else { BUTTON_BG }),
            ))
            .with_children(|field| {
                field.spawn((
                    Text::new(value.to_owned()),
                    body_font(15.0),
                    TextColor(CREAM),
                ));
            });
        });
}

fn action_button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    action: NativeShellButton,
    enabled: bool,
    focused: bool,
) {
    let color = if focused {
        BUTTON_HIGHLIGHT
    } else if !enabled {
        BUTTON_DISABLED
    } else {
        BUTTON_BG
    };
    let mut button = parent.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(34.0),
            display: Display::Flex,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(color),
    ));
    if enabled {
        button.insert((Button, action));
    }
    button.with_children(|label_node| {
        label_node.spawn((
            Text::new(label.to_owned()),
            body_font(16.0),
            TextColor(CREAM),
        ));
    });
}

fn safe_key_button(parent: &mut ChildSpawnerCommands, label: &str, action: NativeShellButton) {
    let mut button = parent.spawn((
        Node {
            width: Val::Px(52.0),
            height: Val::Px(30.0),
            display: Display::Flex,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(BUTTON_BG),
        Button,
        action,
    ));
    button.with_children(|label_node| {
        label_node.spawn((
            Text::new(label.to_owned()),
            body_font(14.0),
            TextColor(CREAM),
        ));
    });
}

fn body_font(size: f32) -> TextFont {
    TextFont {
        font_size: FontSize::Px(size),
        ..default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_password_without_plain_chars() {
        assert_eq!(password_mask(""), "");
        assert_eq!(password_mask("abc"), "***");
        assert_eq!(password_mask("P@ssw0rd"), "********");
    }

    #[test]
    fn printable_chars_filter_control_chars() {
        assert!(is_printable_char('a'));
        assert!(is_printable_char(' '));
        assert!(!is_printable_char('\n'));
    }

    #[test]
    fn focus_cycles_are_consistent() {
        assert!(matches!(
            cycle_login_focus(LoginFocus::Account, false),
            LoginFocus::Password
        ));
        assert!(matches!(
            cycle_login_focus(LoginFocus::Password, true),
            LoginFocus::Account
        ));
        assert!(matches!(
            cycle_login_focus(LoginFocus::LoginButton, false),
            LoginFocus::NewAccountButton
        ));

        assert!(matches!(
            cycle_create_focus(CharacterCreateFocus::Name, false),
            CharacterCreateFocus::Class
        ));
        assert!(matches!(
            cycle_create_focus(CharacterCreateFocus::CancelButton, true),
            CharacterCreateFocus::CreateButton
        ));
    }

    #[test]
    fn rotates_class_and_gender_choices_and_dedups_gateway_intent_queue() {
        assert_eq!(rotate_choice("Warrior", &CLASSES, false), "Wizard");
        assert_eq!(rotate_choice("Female", &GENDERS, true), "Male");

        let mut model = NativeShellModel::default();
        model.screen = NativeShellScreen::Login;
        model.login.account = "hero".to_owned();
        model.login.password = "pwd".to_owned();

        let mut queue = NativeUiIntentQueue::default();
        assert!(apply_and_queue(
            &mut model,
            &mut queue,
            NativeUiIntent::Login
        ));
        let queued = queue.drain().collect::<Vec<_>>();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0], NativeUiIntent::Login);

        model.screen = NativeShellScreen::CharacterSelect;
        let _ = model.apply_ui_intent(NativeUiIntent::OpenCharacterCreate);
        assert!(queue.is_empty());
    }

    #[test]
    fn change_password_submit_intent_contains_authoritative_account_and_all_fields() {
        let mut model = NativeShellModel::default();
        model.change_password.account_id = "account".to_owned();
        model.change_password.old_password = "oldpw".to_owned();
        model.change_password.new_password = "newpw".to_owned();
        model.change_password.confirm_password = "newpw".to_owned();

        assert_eq!(
            change_password_submit_intent(&model),
            NativeUiIntent::SubmitChangePassword {
                account_id: "account".to_owned(),
                old_password: "oldpw".to_owned(),
                new_password: "newpw".to_owned(),
                confirm_password: "newpw".to_owned(),
            }
        );
    }

    #[test]
    fn delete_click_and_cancel_enqueue_no_gateway_packet() {
        let mut model = NativeShellModel::default();
        model.screen = NativeShellScreen::CharacterSelect;
        model.characters = vec![crate::native_shell::CharacterSummary::new(
            4, "Hero", 1, "Warrior", "Male",
        )];
        model.selected_character_index = Some(4);
        let mut queue = NativeUiIntentQueue::default();

        assert!(apply_and_queue(
            &mut model,
            &mut queue,
            NativeUiIntent::DeleteCharacter { character_index: 4 },
        ));
        assert!(queue.is_empty());
        assert_eq!(model.screen, NativeShellScreen::DeleteConfirm { index: 4 });

        assert!(apply_and_queue(
            &mut model,
            &mut queue,
            NativeUiIntent::CancelDeleteCharacter,
        ));
        assert!(queue.is_empty());
        assert_eq!(model.screen, NativeShellScreen::CharacterSelect);
    }
}
