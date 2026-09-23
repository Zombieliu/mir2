#![forbid(unsafe_code)]

use bevy::app::AppExit;
use bevy::ui::{
    widget::NodeImageMode, AlignItems, Display, FlexDirection, JustifyContent, Node, PositionType,
    UiRect, Val,
};
use bevy::{
    input::{
        keyboard::{Key, KeyboardInput},
        ButtonState,
    },
    prelude::*,
};

use crate::crystal_ui::assets::{safe_key_assets, CrystalButtonAssetSet};
use crate::crystal_ui::login::{blink_login_caret, spawn_login_screen, CrystalLoginAction};
use crate::crystal_ui::select::{
    animate_character_previews, spawn_character_preview_at, spawn_character_select_screen,
    CrystalSelectAction,
};
use crate::crystal_ui::spec::{self, CrystalButtonSpec};
use crate::crystal_ui::widget::{
    spawn_crystal_image_button, sync_crystal_image_buttons, CrystalImageButton,
    CrystalImageButtonSprite,
};
use crate::native_shell::{
    parse_registration_birth_date, valid_registration_email, validate_registration_fields,
    ChangePasswordFocus, CharacterCreateFocus, LoginFocus, NativeShellModel, NativeShellScreen,
    NativeUiIntent, NativeUiIntentQueue, RegistrationFocus,
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

const AUX_CHANGE_PASSWORD_PANEL: spec::CrystalRect =
    spec::CrystalRect::new(348.0, 224.0, 328.0, 350.0);
const AUX_CONFIRM_PANEL: spec::CrystalRect = spec::CrystalRect::new(348.0, 286.0, 328.0, 196.0);
const CONNECTION_LOST_PANEL: spec::CrystalRect = spec::CrystalRect::new(284.0, 289.0, 456.0, 190.0);

const NEW_CHARACTER_FRAME: spec::CrystalRect = spec::CrystalRect::new(218.0, 154.0, 588.0, 460.0);
const NEW_CHARACTER_TITLE: spec::CrystalRect = spec::CrystalRect::new(424.0, 165.0, 187.0, 20.0);
const NEW_CHARACTER_NAME_FIELD: spec::CrystalRect =
    spec::CrystalRect::new(543.0, 422.0, 240.0, 20.0);
const NEW_CHARACTER_PREVIEW_ANCHOR: (f32, f32) = (338.0, 404.0);
const NEW_CHARACTER_CLASS_BUTTONS: [spec::CrystalRect; 3] = [
    spec::CrystalRect::new(541.0, 450.0, 44.0, 42.0),
    spec::CrystalRect::new(591.0, 450.0, 44.0, 42.0),
    spec::CrystalRect::new(641.0, 450.0, 44.0, 42.0),
];
const NEW_CHARACTER_GENDER_BUTTONS: [spec::CrystalRect; 2] = [
    spec::CrystalRect::new(541.0, 497.0, 44.0, 42.0),
    spec::CrystalRect::new(591.0, 497.0, 44.0, 42.0),
];
const NEW_CHARACTER_CREATE: spec::CrystalRect = spec::CrystalRect::new(378.0, 579.0, 100.0, 25.0);
const NEW_CHARACTER_CANCEL: spec::CrystalRect = spec::CrystalRect::new(643.0, 579.0, 100.0, 25.0);
const NEW_ACCOUNT_FRAME: spec::CrystalRect = spec::CrystalRect::new(218.0, 154.0, 588.0, 460.0);
const NEW_ACCOUNT_FIELDS: [(RegistrationFocus, spec::CrystalRect); 8] = [
    (RegistrationFocus::AccountId, spec::CrystalRect::new(444.0, 257.0, 136.0, 18.0)),
    (RegistrationFocus::Password, spec::CrystalRect::new(444.0, 283.0, 136.0, 18.0)),
    (RegistrationFocus::ConfirmPassword, spec::CrystalRect::new(444.0, 309.0, 136.0, 18.0)),
    (RegistrationFocus::UserName, spec::CrystalRect::new(444.0, 343.0, 136.0, 18.0)),
    (RegistrationFocus::BirthDate, spec::CrystalRect::new(444.0, 369.0, 136.0, 18.0)),
    (RegistrationFocus::SecretQuestion, spec::CrystalRect::new(444.0, 404.0, 190.0, 18.0)),
    (RegistrationFocus::SecretAnswer, spec::CrystalRect::new(444.0, 430.0, 190.0, 18.0)),
    (RegistrationFocus::EmailAddress, spec::CrystalRect::new(444.0, 465.0, 136.0, 18.0)),
];
const NEW_ACCOUNT_DESCRIPTION: spec::CrystalRect =
    spec::CrystalRect::new(233.0, 494.0, 300.0, 70.0);
const NEW_ACCOUNT_OK: spec::CrystalRect = spec::CrystalRect::new(353.0, 579.0, 76.0, 25.0);
const NEW_ACCOUNT_CANCEL: spec::CrystalRect = spec::CrystalRect::new(627.0, 579.0, 76.0, 25.0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub(crate) enum NativeShellField {
    CharacterName,
    ChangePassword(ChangePasswordFocus),
    Registration(RegistrationFocus),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum DeleteConfirmFocus {
    #[default]
    Confirm,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Resource)]
struct NativeShellAuxFocus {
    delete_confirm: DeleteConfirmFocus,
    connection_retry: bool,
}

#[derive(Debug, Clone, Copy, Default, Resource)]
struct NativeShellTextModifiers {
    control: bool,
    alt: bool,
    super_key: bool,
}

impl NativeShellTextModifiers {
    fn observe(&mut self, event: &KeyboardInput) {
        let pressed = event.state == ButtonState::Pressed;
        match event.key_code {
            KeyCode::ControlLeft | KeyCode::ControlRight => self.control = pressed,
            KeyCode::AltLeft | KeyCode::AltRight => self.alt = pressed,
            KeyCode::SuperLeft | KeyCode::SuperRight => self.super_key = pressed,
            _ => {}
        }
    }

    fn command_modified(self) -> bool {
        self.control || self.alt || self.super_key
    }

    fn suppresses_composed_text(self) -> bool {
        // Ctrl/Super shortcuts must never become editable-field contents even
        // when winit supplies `KeyboardInput::text`. Alt is deliberately not
        // included because AltGr/dead-key layouts rely on composed text.
        self.control || self.super_key
    }
}

fn collect_typed_text<'a>(
    events: impl IntoIterator<Item = &'a KeyboardInput>,
    modifiers: &mut NativeShellTextModifiers,
) -> String {
    let mut typed = String::new();
    for event in events {
        modifiers.observe(event);
        if event.state != ButtonState::Pressed {
            continue;
        }

        if modifiers.suppresses_composed_text() {
            continue;
        }

        if let Some(text) = event.text.as_deref().filter(|text| !text.is_empty()) {
            // Composed text is winit's authoritative human-input result. This
            // preserves AltGr/dead-key layouts. Ctrl/Super shortcuts are
            // filtered above because Windows may still attach composed text.
            typed.push_str(text);
        } else if !modifiers.command_modified() {
            let Key::Character(text) = &event.logical_key else {
                continue;
            };
            // SendInput-style automation may provide the logical character but
            // omit winit's composed `text`. Human keyboard input normally uses
            // `text`; this fallback keeps both paths lossless without turning
            // Ctrl/Alt/Super shortcuts into field contents.
            typed.push_str(text);
        }
    }
    typed
}

fn cycle_delete_focus(current: DeleteConfirmFocus, _reverse: bool) -> DeleteConfirmFocus {
    // This dialog has exactly two focus targets. Forward and reverse traversal
    // therefore both move to the other target; direction only becomes
    // meaningful once a third focusable control exists.
    match current {
        DeleteConfirmFocus::Confirm => DeleteConfirmFocus::Cancel,
        DeleteConfirmFocus::Cancel => DeleteConfirmFocus::Confirm,
    }
}

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

pub fn cycle_registration_focus(current: RegistrationFocus, reverse: bool) -> RegistrationFocus {
    use RegistrationFocus::*;
    match (current, reverse) {
        (AccountId, false) => Password,
        (Password, false) => ConfirmPassword,
        (ConfirmPassword, false) => UserName,
        (UserName, false) => BirthDate,
        (BirthDate, false) => SecretQuestion,
        (SecretQuestion, false) => SecretAnswer,
        (SecretAnswer, false) => EmailAddress,
        (EmailAddress, false) => SubmitButton,
        (SubmitButton, false) => CancelButton,
        (CancelButton, false) => AccountId,
        (AccountId, true) => CancelButton,
        (Password, true) => AccountId,
        (ConfirmPassword, true) => Password,
        (UserName, true) => ConfirmPassword,
        (BirthDate, true) => UserName,
        (SecretQuestion, true) => BirthDate,
        (SecretAnswer, true) => SecretQuestion,
        (EmailAddress, true) => SecretAnswer,
        (SubmitButton, true) => EmailAddress,
        (CancelButton, true) => SubmitButton,
    }
}

fn is_gateway_intent(intent: &NativeUiIntent) -> bool {
    matches!(
        intent,
        NativeUiIntent::Login
            | NativeUiIntent::SubmitRegistration { .. }
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
    // The model owns logical-operation in-flight state.  Do not use the
    // queue's global emptiness as a gate: a pending Register must not swallow
    // a distinct Login, and draining the queue must not reopen a request
    // before its ACK/failure arrives.
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

fn registration_submit_intent(model: &NativeShellModel) -> Result<NativeUiIntent, &'static str> {
    Ok(NativeUiIntent::SubmitRegistration {
        account_id: model.registration.account_id.clone(),
        password: model.registration.password.clone(),
        confirm_password: model.registration.confirm_password.clone(),
        birth_date: model.registration.birth_date.clone(),
        birth_date_binary: parse_registration_birth_date(&model.registration.birth_date)?,
        user_name: model.registration.user_name.clone(),
        secret_question: model.registration.secret_question.clone(),
        secret_answer: model.registration.secret_answer.clone(),
        email_address: model.registration.email_address.clone(),
    })
}

fn registration_submit_enabled(model: &NativeShellModel) -> bool {
    !model.register_request_in_flight && validate_registration_fields(&model.registration).is_ok()
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
pub(crate) enum NativeShellButton {
    CancelCreate,
    SubmitCreate,
    SelectCreateClass(u8),
    SelectCreateGender(u8),
    Retry,
    CycleClass,
    CycleGender,
    ConfirmDelete,
    CancelDelete,
    SubmitChangePassword,
    CancelChangePassword,
    SubmitRegistration,
    CancelRegistration,
    CloseSafeKey,
    SafeKeyFocusAccount,
    SafeKeyFocusPassword,
    SafeKeyPress(char),
    SafeKeyDelete,
    SafeKeyRandom,
    SafeKeyEnter,
}

pub struct Mir2NativeShellUiPlugin;

#[derive(Component)]
struct NativeLoginDoorBackground;

#[derive(Resource)]
struct NativeLoginDoorFrames(Vec<Handle<Image>>);

fn animate_login_door(
    time: Res<Time>,
    mut shell: Option<ResMut<NativeShellModel>>,
    frames: Option<Res<NativeLoginDoorFrames>>,
    images: Res<Assets<Image>>,
    mut backgrounds: Query<&mut ImageNode, With<NativeLoginDoorBackground>>,
) {
    let (Some(shell), Some(frames)) = (shell.as_deref_mut(), frames) else {
        return;
    };
    // Keep the first frame visible until all frames are resident. Do not skip
    // the opening sequence on a cold disk or flash missing image placeholders.
    let ready = frames.0.iter().all(|frame| images.contains(frame.id()));
    if ready {
        shell.advance_login_opening(time.delta());
    }
    let frame = if ready {
        shell.login_opening_frame() as usize
    } else {
        0
    };
    for mut image in &mut backgrounds {
        if image.image != frames.0[frame] {
            image.image = frames.0[frame].clone();
        }
    }
}

impl Plugin for Mir2NativeShellUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NativeUiIntentQueue>()
            .init_resource::<NativeShellAuxFocus>()
            .init_resource::<NativeShellTextModifiers>()
            .add_systems(Startup, spawn_shell_ui)
            .add_systems(
                Update,
                (
                    animate_login_door,
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
    commands.insert_resource(NativeLoginDoorFrames(
        (0..19)
            .map(|index| asset_server.load(format!("original-ui/ChrSel/{index}.png")))
            .collect(),
    ));
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
                NativeLoginDoorBackground,
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
    field_interactions: Query<
        (&Interaction, &NativeShellField),
        (Changed<Interaction>, With<Button>),
    >,
    shell: Option<ResMut<NativeShellModel>>,
    queue: Option<ResMut<NativeUiIntentQueue>>,
    aux_focus: Option<ResMut<NativeShellAuxFocus>>,
    mut app_exit: MessageWriter<AppExit>,
) {
    let (Some(mut shell), Some(mut queue), Some(mut aux_focus)) = (shell, queue, aux_focus) else {
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
                let _ = shell.apply_ui_intent(NativeUiIntent::OpenRegistration);
            }
            CrystalLoginAction::ChangePassword => {
                let _ = shell.apply_ui_intent(NativeUiIntent::OpenChangePassword);
            }
            CrystalLoginAction::SafeKey => {
                let _ = shell.apply_ui_intent(NativeUiIntent::OpenSafeKey);
            }
            CrystalLoginAction::Cancel => {
                eprintln!("[native-lifecycle] exit_source=login_cancel");
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
                    if apply_and_queue(
                        &mut shell,
                        &mut queue,
                        NativeUiIntent::DeleteCharacter { character_index },
                    ) {
                        aux_focus.delete_confirm = DeleteConfirmFocus::Confirm;
                    }
                }
            }
            // Crystal's SelectScene credits handler is intentionally empty.
            CrystalSelectAction::Credits => {}
            CrystalSelectAction::Exit => {
                eprintln!("[native-lifecycle] exit_source=character_select_exit");
                app_exit.write(AppExit::Success);
            }
        }
    }

    for (interaction, field) in field_interactions.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match field {
            NativeShellField::CharacterName
                if shell.screen == NativeShellScreen::CharacterCreate =>
            {
                shell.character_create.focus = CharacterCreateFocus::Name;
            }
            NativeShellField::ChangePassword(focus)
                if shell.screen == NativeShellScreen::ChangePassword
                    && !shell.change_password_request_in_flight =>
            {
                shell.change_password.focus = *focus;
            }
            NativeShellField::Registration(focus)
                if shell.screen == NativeShellScreen::Registration
                    && !shell.register_request_in_flight =>
            {
                shell.registration.focus = *focus;
            }
            _ => {}
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
            NativeShellButton::SelectCreateClass(class_index) => {
                if let Some(class_name) = CLASSES.get(*class_index as usize) {
                    shell.character_create.focus = CharacterCreateFocus::Class;
                    shell.character_create.class_name = (*class_name).to_owned();
                }
            }
            NativeShellButton::SelectCreateGender(gender_index) => {
                if let Some(gender_name) = GENDERS.get(*gender_index as usize) {
                    shell.character_create.focus = CharacterCreateFocus::Gender;
                    shell.character_create.gender_name = (*gender_name).to_owned();
                }
            }
            NativeShellButton::Retry => {
                aux_focus.connection_retry = true;
                apply_and_queue(&mut shell, &mut queue, NativeUiIntent::Retry);
            }
            NativeShellButton::CycleClass => {
                shell.character_create.focus = CharacterCreateFocus::Class;
                shell.character_create.class_name =
                    rotate_choice(&shell.character_create.class_name, &CLASSES, false);
            }
            NativeShellButton::CycleGender => {
                shell.character_create.focus = CharacterCreateFocus::Gender;
                shell.character_create.gender_name =
                    rotate_choice(&shell.character_create.gender_name, &GENDERS, false);
            }
            NativeShellButton::ConfirmDelete => {
                aux_focus.delete_confirm = DeleteConfirmFocus::Confirm;
                apply_and_queue(
                    &mut shell,
                    &mut queue,
                    NativeUiIntent::ConfirmDeleteCharacter,
                );
            }
            NativeShellButton::CancelDelete => {
                aux_focus.delete_confirm = DeleteConfirmFocus::Cancel;
                let _ = shell.apply_ui_intent(NativeUiIntent::CancelDeleteCharacter);
            }
            NativeShellButton::SubmitChangePassword => {
                let intent = change_password_submit_intent(&shell);
                apply_and_queue(&mut shell, &mut queue, intent);
            }
            NativeShellButton::CancelChangePassword => {
                let _ = shell.apply_ui_intent(NativeUiIntent::CancelChangePassword);
            }
            NativeShellButton::SubmitRegistration => {
                if let Ok(intent) = registration_submit_intent(&shell) {
                    apply_and_queue(&mut shell, &mut queue, intent);
                } else {
                    shell.notice = Some(crate::native_shell::ShellNotice::error(
                        "birth date must use YYYY-MM-DD",
                    ));
                }
            }
            NativeShellButton::CancelRegistration => {
                let _ = shell.apply_ui_intent(NativeUiIntent::CancelRegistration);
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
    aux_focus: Option<ResMut<NativeShellAuxFocus>>,
    modifiers: Option<ResMut<NativeShellTextModifiers>>,
) {
    let (Some(mut shell), Some(mut queue), Some(mut aux_focus), Some(mut modifiers)) =
        (shell, queue, aux_focus, modifiers)
    else {
        return;
    };

    // Drain the complete ordered queue every frame. Rapid SendInput bursts can
    // place several characters in one Bevy update; consuming a single event or
    // relying only on `KeyboardInput::text` loses synthetic characters.
    // ButtonInput deliberately collapses OS key-repeat presses, so retain the
    // raw messages for editable-field Backspace/Delete repeat semantics.
    let keyboard_events = keyboard_inputs.read().collect::<Vec<_>>();
    let edit_delete_count =
        editable_key_press_count(&keyboard_events, &keys, KeyCode::Backspace).saturating_add(
            editable_key_press_count(&keyboard_events, &keys, KeyCode::Delete),
        );
    let typed_text = collect_typed_text(keyboard_events.iter().copied(), &mut modifiers);
    modifiers.control = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    modifiers.alt = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
    modifiers.super_key = keys.pressed(KeyCode::SuperLeft) || keys.pressed(KeyCode::SuperRight);

    if shell.screen == NativeShellScreen::InGame {
        return;
    }
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
                        let _ = shell.apply_ui_intent(NativeUiIntent::OpenRegistration);
                    }
                }
                return;
            }

            match shell.login.focus {
                LoginFocus::Account => {
                    pop_editable_tail(&mut shell.login.account, edit_delete_count);
                }
                LoginFocus::Password => {
                    pop_editable_tail(&mut shell.login.password, edit_delete_count);
                }
                LoginFocus::LoginButton | LoginFocus::NewAccountButton => {}
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

            if matches!(shell.character_create.focus, CharacterCreateFocus::Name) {
                pop_editable_tail(&mut shell.character_create.name, edit_delete_count);
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
                    append_name_field(&mut shell.character_create.name, c, MAX_NAME);
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

        NativeShellScreen::Registration => {
            if keys.just_pressed(KeyCode::Tab) {
                shell.registration.focus =
                    cycle_registration_focus(shell.registration.focus, shifted);
                return;
            }
            if keys.just_pressed(KeyCode::Escape) {
                let _ = shell.apply_ui_intent(NativeUiIntent::CancelRegistration);
                return;
            }
            if keys.just_pressed(KeyCode::Enter) {
                match shell.registration.focus {
                    RegistrationFocus::SubmitButton => match registration_submit_intent(&shell) {
                        Ok(intent) => {
                            apply_and_queue(&mut shell, &mut queue, intent);
                        }
                        Err(message) => shell.notice = Some(
                            crate::native_shell::ShellNotice::error(message),
                        ),
                    },
                    RegistrationFocus::CancelButton => {
                        let _ = shell.apply_ui_intent(NativeUiIntent::CancelRegistration);
                    }
                    focus => shell.registration.focus = cycle_registration_focus(focus, false),
                }
                return;
            }
            if shell.register_request_in_flight {
                return;
            }
            match shell.registration.focus {
                RegistrationFocus::AccountId => {
                    pop_editable_tail(&mut shell.registration.account_id, edit_delete_count)
                }
                RegistrationFocus::Password => {
                    pop_editable_tail(&mut shell.registration.password, edit_delete_count)
                }
                RegistrationFocus::ConfirmPassword => pop_editable_tail(
                    &mut shell.registration.confirm_password,
                    edit_delete_count,
                ),
                RegistrationFocus::UserName => {
                    pop_editable_tail(&mut shell.registration.user_name, edit_delete_count)
                }
                RegistrationFocus::BirthDate => {
                    pop_editable_tail(&mut shell.registration.birth_date, edit_delete_count)
                }
                RegistrationFocus::SecretQuestion => pop_editable_tail(
                    &mut shell.registration.secret_question,
                    edit_delete_count,
                ),
                RegistrationFocus::SecretAnswer => pop_editable_tail(
                    &mut shell.registration.secret_answer,
                    edit_delete_count,
                ),
                RegistrationFocus::EmailAddress => {
                    pop_editable_tail(&mut shell.registration.email_address, edit_delete_count)
                }
                RegistrationFocus::SubmitButton | RegistrationFocus::CancelButton => {}
            }
            for c in typed_text.chars() {
                match shell.registration.focus {
                    RegistrationFocus::AccountId => append_alphanumeric_field(
                        &mut shell.registration.account_id,
                        c,
                        MAX_CHANGE_ACCOUNT,
                    ),
                    RegistrationFocus::Password => append_alphanumeric_field(
                        &mut shell.registration.password,
                        c,
                        MAX_CHANGE_PASSWORD,
                    ),
                    RegistrationFocus::ConfirmPassword => append_alphanumeric_field(
                        &mut shell.registration.confirm_password,
                        c,
                        MAX_CHANGE_PASSWORD,
                    ),
                    RegistrationFocus::UserName => {
                        append_editable_field(&mut shell.registration.user_name, c, 20)
                    }
                    RegistrationFocus::BirthDate => {
                        if c.is_ascii_digit() || c == '-' {
                            append_editable_field(&mut shell.registration.birth_date, c, 10)
                        }
                    }
                    RegistrationFocus::SecretQuestion => {
                        append_editable_field(&mut shell.registration.secret_question, c, 30)
                    }
                    RegistrationFocus::SecretAnswer => {
                        append_editable_field(&mut shell.registration.secret_answer, c, 30)
                    }
                    RegistrationFocus::EmailAddress => {
                        if !c.is_control() {
                            append_editable_field(&mut shell.registration.email_address, c, 50)
                        }
                    }
                    RegistrationFocus::SubmitButton | RegistrationFocus::CancelButton => {}
                }
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
            match shell.change_password.focus {
                ChangePasswordFocus::AccountId => {
                    pop_editable_tail(&mut shell.change_password.account_id, edit_delete_count);
                }
                ChangePasswordFocus::OldPassword => {
                    pop_editable_tail(&mut shell.change_password.old_password, edit_delete_count);
                }
                ChangePasswordFocus::NewPassword => {
                    pop_editable_tail(&mut shell.change_password.new_password, edit_delete_count);
                }
                ChangePasswordFocus::ConfirmPassword => {
                    pop_editable_tail(
                        &mut shell.change_password.confirm_password,
                        edit_delete_count,
                    );
                }
                _ => {}
            }
            for c in typed_text.chars() {
                match shell.change_password.focus {
                    ChangePasswordFocus::AccountId => append_alphanumeric_field(
                        &mut shell.change_password.account_id,
                        c,
                        MAX_CHANGE_ACCOUNT,
                    ),
                    ChangePasswordFocus::OldPassword => append_alphanumeric_field(
                        &mut shell.change_password.old_password,
                        c,
                        MAX_CHANGE_PASSWORD,
                    ),
                    ChangePasswordFocus::NewPassword => append_alphanumeric_field(
                        &mut shell.change_password.new_password,
                        c,
                        MAX_CHANGE_PASSWORD,
                    ),
                    ChangePasswordFocus::ConfirmPassword => append_alphanumeric_field(
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
            if edit_delete_count > 0 {
                for _ in 0..edit_delete_count {
                    let _ = shell.apply_ui_intent(NativeUiIntent::SafeKeyDelete);
                }
                return;
            }
            if keys.just_pressed(KeyCode::Enter) {
                apply_and_queue(&mut shell, &mut queue, NativeUiIntent::SafeKeyEnter);
                return;
            }
        }
        NativeShellScreen::DeleteConfirm { .. } => {
            if keys.just_pressed(KeyCode::Tab) {
                aux_focus.delete_confirm = cycle_delete_focus(aux_focus.delete_confirm, shifted);
                return;
            }
            if keys.just_pressed(KeyCode::Escape) {
                let _ = shell.apply_ui_intent(NativeUiIntent::CancelDeleteCharacter);
                return;
            }
            if keys.just_pressed(KeyCode::Enter) {
                match aux_focus.delete_confirm {
                    DeleteConfirmFocus::Confirm => apply_and_queue(
                        &mut shell,
                        &mut queue,
                        NativeUiIntent::ConfirmDeleteCharacter,
                    ),
                    DeleteConfirmFocus::Cancel => {
                        shell.apply_ui_intent(NativeUiIntent::CancelDeleteCharacter)
                    }
                };
                return;
            }
        }
        NativeShellScreen::ConnectionLost => {
            if keys.just_pressed(KeyCode::Tab) {
                aux_focus.connection_retry = true;
                return;
            }
            if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Escape) {
                aux_focus.connection_retry = true;
                apply_and_queue(&mut shell, &mut queue, NativeUiIntent::Retry);
            }
        }

        _ => {}
    }
}

fn editable_key_press_count(
    events: &[&KeyboardInput],
    keys: &ButtonInput<KeyCode>,
    key_code: KeyCode,
) -> usize {
    let raw_press_count = events
        .iter()
        .filter(|event| event.key_code == key_code && event.state == ButtonState::Pressed)
        .count();
    if raw_press_count > 0 {
        raw_press_count
    } else {
        usize::from(keys.just_pressed(key_code))
    }
}

fn pop_editable_tail(text: &mut String, count: usize) {
    // These Crystal fields do not yet expose a cursor or selection model.
    // Until that denominator is implemented, both Backspace and Delete keep
    // the existing tail-delete behavior while honoring every OS repeat event.
    for _ in 0..count {
        text.pop();
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

fn append_name_field(text: &mut String, c: char, max: usize) {
    if !is_printable_char(c) || c.is_whitespace() {
        return;
    }
    if text.chars().count() >= max {
        return;
    }
    text.push(c);
}

fn append_alphanumeric_field(text: &mut String, c: char, max: usize) {
    if !c.is_ascii_alphanumeric() {
        return;
    }
    append_editable_field(text, c, max);
}

fn render_shell_ui(
    mut commands: Commands,
    model: Option<Res<NativeShellModel>>,
    asset_server: Res<AssetServer>,
    aux_focus: Option<Res<NativeShellAuxFocus>>,
    content_nodes: Query<Entity, With<NativeShellContent>>,
    mut last_rendered_model: Local<Option<NativeShellModel>>,
    mut last_rendered_aux_focus: Local<Option<NativeShellAuxFocus>>,
) {
    let Some(model) = model else {
        return;
    };

    if model.screen == NativeShellScreen::InGame {
        return;
    }

    let current_aux_focus = aux_focus.as_deref().copied().unwrap_or_default();

    if last_rendered_model.as_ref() == Some(model.as_ref())
        && last_rendered_aux_focus.as_ref() == Some(&current_aux_focus)
    {
        return;
    }
    *last_rendered_model = Some(model.clone());
    *last_rendered_aux_focus = Some(current_aux_focus);

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
            NativeShellScreen::OpeningLogin => {
                // The original login dialog is disposed before the door opens.
                // No widgets or generic loading panel may obscure the frames.
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
            NativeShellScreen::CharacterCreate => {
                render_character_create(screen, &asset_server, &model);
            }
            NativeShellScreen::Registration => {
                render_registration(screen, &asset_server, &model);
            }
            NativeShellScreen::ChangePassword => {
                render_change_password(screen, &asset_server, &model);
            }
            NativeShellScreen::SafeKey => {
                spawn_login_screen(
                    screen,
                    &asset_server,
                    &model,
                    &password_mask(&model.login.password),
                );
                render_safe_key(screen, &asset_server, &model);
            }
            NativeShellScreen::DeleteConfirm { index } => {
                render_delete_confirm(screen, &asset_server, &model, index, current_aux_focus);
            }
            NativeShellScreen::ConnectionLost => {
                render_connection_lost(screen, &asset_server, &model, current_aux_focus);
            }
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

fn render_character_create(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    model: &NativeShellModel,
) {
    // Crystal's NewCharacterDialog is a 588x460 modal at (218,154).  The
    // frame already contains the input border and the five class/two gender
    // slots, so the native overlay must be anchored to that frame rather than
    // recreating a second generic panel beside it.
    parent.spawn((
        absolute_node(NEW_CHARACTER_FRAME),
        ImageNode {
            image: asset_server.load("original-ui/Prguse/73.png"),
            image_mode: NodeImageMode::Stretch,
            ..default()
        },
    ));
    spawn_native_image(
        parent,
        asset_server,
        "original-ui/Title/20.png",
        NEW_CHARACTER_TITLE,
    );
    spawn_character_preview_at(
        parent,
        asset_server,
        &model.character_create.class_name,
        &model.character_create.gender_name,
        NEW_CHARACTER_PREVIEW_ANCHOR,
    );
    spawn_aux_text(
        parent,
        character_description(&model.character_create.class_name),
        spec::CrystalRect::new(497.0, 224.0, 278.0, 170.0),
        13.0,
        CREAM,
        Justify::Left,
    );
    spawn_crystal_name_field(
        parent,
        NEW_CHARACTER_NAME_FIELD,
        &model.character_create.name,
        model.character_create.focus == CharacterCreateFocus::Name,
        true,
        NativeShellField::CharacterName,
    );

    for (index, rect) in NEW_CHARACTER_CLASS_BUTTONS.into_iter().enumerate() {
        let selected = character_class_index(&model.character_create.class_name) == index as u16;
        let normal = 2426 + index as u16 * 3 + if selected { 1 } else { 0 };
        let class_spec = CrystalButtonSpec::new(
            "Prguse",
            normal,
            normal + if selected { 0 } else { 1 },
            2428 + index as u16 * 3,
            rect,
            44.0,
            42.0,
        );
        spawn_crystal_image_button(
            parent,
            asset_server,
            class_spec,
            CrystalButtonAssetSet::from_spec(class_spec),
            NativeShellButton::SelectCreateClass(index as u8),
            model.character_create.focus == CharacterCreateFocus::Class && selected,
            true,
        );
    }
    for (index, rect) in NEW_CHARACTER_GENDER_BUTTONS.into_iter().enumerate() {
        let selected = character_gender_index(&model.character_create.gender_name) == index as u16;
        let base = if index == 0 { 2420 } else { 2423 };
        let normal = base + if selected { 1 } else { 0 };
        let gender_spec = CrystalButtonSpec::new(
            "Prguse",
            normal,
            normal + if selected { 0 } else { 1 },
            base + 2,
            rect,
            44.0,
            42.0,
        );
        spawn_crystal_image_button(
            parent,
            asset_server,
            gender_spec,
            CrystalButtonAssetSet::from_spec(gender_spec),
            NativeShellButton::SelectCreateGender(index as u8),
            model.character_create.focus == CharacterCreateFocus::Gender && selected,
            true,
        );
    }

    if let Some(notice) = &model.notice {
        spawn_aux_notice(
            parent,
            &notice.message,
            notice.kind,
            spec::CrystalRect::new(628.0, 610.0, 304.0, 28.0),
        );
    }
    let create_spec =
        CrystalButtonSpec::new("Title", 360, 361, 362, NEW_CHARACTER_CREATE, 100.0, 25.0);
    let cancel_spec = spec::login::CANCEL;
    spawn_crystal_image_button(
        parent,
        asset_server,
        create_spec,
        CrystalButtonAssetSet::from_spec(create_spec),
        NativeShellButton::SubmitCreate,
        model.character_create.focus == CharacterCreateFocus::CreateButton,
        model.character_create.is_ready(),
    );
    spawn_crystal_image_button(
        parent,
        asset_server,
        CrystalButtonSpec::new(
            cancel_spec.library,
            cancel_spec.normal,
            cancel_spec.hover,
            cancel_spec.pressed,
            NEW_CHARACTER_CANCEL,
            100.0,
            25.0,
        ),
        CrystalButtonAssetSet::from_spec(cancel_spec),
        NativeShellButton::CancelCreate,
        model.character_create.focus == CharacterCreateFocus::CancelButton,
        true,
    );
}

fn render_registration(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    model: &NativeShellModel,
) {
    // Crystal's LoginScene.NewAccountDialog uses Prguse frame 63 at the centre
    // of the 1024x768 stage.  Its labels are part of the exported source art;
    // only its text boxes, description, and Title buttons are live controls.
    spawn_native_image(parent, asset_server, "original-ui/Prguse/63.png", NEW_ACCOUNT_FRAME);

    let form = &model.registration;
    let password = password_mask(&form.password);
    let confirm_password = password_mask(&form.confirm_password);
    for (focus, rect) in NEW_ACCOUNT_FIELDS {
        let value = match focus {
            RegistrationFocus::AccountId => form.account_id.as_str(),
            RegistrationFocus::Password => password.as_str(),
            RegistrationFocus::ConfirmPassword => confirm_password.as_str(),
            RegistrationFocus::UserName => form.user_name.as_str(),
            RegistrationFocus::BirthDate => form.birth_date.as_str(),
            RegistrationFocus::SecretQuestion => form.secret_question.as_str(),
            RegistrationFocus::SecretAnswer => form.secret_answer.as_str(),
            RegistrationFocus::EmailAddress => form.email_address.as_str(),
            RegistrationFocus::SubmitButton | RegistrationFocus::CancelButton => unreachable!(),
        };
        spawn_registration_field(
            parent,
            rect,
            value,
            !model.register_request_in_flight,
            focus,
            registration_field_border_color(form, focus),
        );
    }

    spawn_aux_text(
        parent,
        registration_description(form.focus),
        NEW_ACCOUNT_DESCRIPTION,
        11.0,
        CREAM,
        Justify::Left,
    );
    if let Some(notice) = &model.notice {
        spawn_aux_notice(
            parent,
            &notice.message,
            notice.kind,
            spec::CrystalRect::new(538.0, 494.0, 252.0, 70.0),
        );
    }

    let submit_spec = CrystalButtonSpec::new("Title", 200, 201, 202, NEW_ACCOUNT_OK, 76.0, 25.0);
    let cancel_spec =
        CrystalButtonSpec::new("Title", 203, 204, 205, NEW_ACCOUNT_CANCEL, 76.0, 25.0);
    spawn_crystal_image_button(
        parent,
        asset_server,
        submit_spec,
        CrystalButtonAssetSet::from_spec(submit_spec),
        NativeShellButton::SubmitRegistration,
        form.focus == RegistrationFocus::SubmitButton,
        registration_submit_enabled(model),
    );
    spawn_crystal_image_button(
        parent,
        asset_server,
        cancel_spec,
        CrystalButtonAssetSet::from_spec(cancel_spec),
        NativeShellButton::CancelRegistration,
        form.focus == RegistrationFocus::CancelButton,
        !model.register_request_in_flight,
    );
}

fn spawn_registration_field(
    parent: &mut ChildSpawnerCommands,
    rect: spec::CrystalRect,
    value: &str,
    enabled: bool,
    focus: RegistrationFocus,
    border_color: Color,
) {
    let mut node = absolute_node(rect);
    node.border = UiRect::all(Val::Px(1.0));
    node.padding = UiRect::horizontal(Val::Px(2.0));
    node.overflow = Overflow::clip();
    node.align_items = AlignItems::Center;
    let mut field = parent.spawn((
        node,
        BackgroundColor(Color::BLACK),
        BorderColor::all(border_color),
        NativeShellField::Registration(focus),
    ));
    if enabled {
        field.insert(Button);
    }
    field.with_children(|field| {
        field.spawn((
            Text::new(value.to_owned()),
            body_font(11.0),
            TextColor(Color::WHITE),
        ));
    });
}

fn registration_field_border_color(
    form: &crate::native_shell::RegistrationForm,
    focus: RegistrationFocus,
) -> Color {
    let neutral = Color::srgb_u8(128, 128, 128);
    let valid = Color::srgb(0.0, 0.5, 0.0);
    let invalid = Color::srgb(0.75, 0.0, 0.0);
    let alphanumeric = |value: &str, min, max| {
        let length = value.chars().count();
        (min..=max).contains(&length) && value.chars().all(|character| character.is_ascii_alphanumeric())
    };
    match focus {
        RegistrationFocus::AccountId => {
            if form.account_id.is_empty() {
                neutral
            } else if alphanumeric(&form.account_id, 3, 15) {
                valid
            } else {
                invalid
            }
        }
        RegistrationFocus::Password => {
            if form.password.is_empty() {
                neutral
            } else if alphanumeric(&form.password, 5, 15) {
                valid
            } else {
                invalid
            }
        }
        RegistrationFocus::ConfirmPassword => {
            if form.confirm_password.is_empty() {
                neutral
            } else if alphanumeric(&form.confirm_password, 5, 15)
                && form.confirm_password == form.password
            {
                valid
            } else {
                invalid
            }
        }
        RegistrationFocus::UserName => optional_field_color(&form.user_name, 20, neutral, valid, invalid),
        RegistrationFocus::BirthDate => {
            if form.birth_date.is_empty() {
                neutral
            } else if parse_registration_birth_date(&form.birth_date).is_ok() {
                valid
            } else {
                invalid
            }
        }
        RegistrationFocus::SecretQuestion => {
            optional_field_color(&form.secret_question, 30, neutral, valid, invalid)
        }
        RegistrationFocus::SecretAnswer => {
            optional_field_color(&form.secret_answer, 30, neutral, valid, invalid)
        }
        RegistrationFocus::EmailAddress => {
            if form.email_address.is_empty() {
                neutral
            } else if form.email_address.chars().count() <= 50
                && valid_registration_email(&form.email_address)
            {
                valid
            } else {
                invalid
            }
        }
        RegistrationFocus::SubmitButton | RegistrationFocus::CancelButton => neutral,
    }
}

fn optional_field_color(
    value: &str,
    max_chars: usize,
    neutral: Color,
    valid: Color,
    invalid: Color,
) -> Color {
    if value.is_empty() {
        neutral
    } else if value.chars().count() <= max_chars {
        valid
    } else {
        invalid
    }
}

fn registration_description(focus: RegistrationFocus) -> &'static str {
    match focus {
        RegistrationFocus::AccountId => "Account ID: 3-15 letters or numbers.",
        RegistrationFocus::Password | RegistrationFocus::ConfirmPassword => {
            "Password: 5-15 letters or numbers. Confirm it exactly."
        }
        RegistrationFocus::UserName => "Optional user name: up to 20 characters.",
        RegistrationFocus::BirthDate => "Optional birth date: YYYY-MM-DD.",
        RegistrationFocus::SecretQuestion => "Optional secret question: up to 30 characters.",
        RegistrationFocus::SecretAnswer => "Optional secret answer: up to 30 characters.",
        RegistrationFocus::EmailAddress => "Optional email address: up to 50 characters.",
        RegistrationFocus::SubmitButton => "Create the account with these details.",
        RegistrationFocus::CancelButton => "Return to the login screen.",
    }
}

fn render_change_password(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    model: &NativeShellModel,
) {
    crate::crystal_ui::change_password::spawn_change_password(parent, asset_server, model);
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

fn render_delete_confirm(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    model: &NativeShellModel,
    index: i32,
    aux_focus: NativeShellAuxFocus,
) {
    let name = model
        .characters
        .iter()
        .find(|c| c.index == index)
        .map(|c| c.name.as_str())
        .unwrap_or("Unknown");
    spawn_auxiliary_panel(parent, asset_server, AUX_CONFIRM_PANEL);
    spawn_aux_text(
        parent,
        "Delete Character",
        spec::CrystalRect::new(366.0, 304.0, 292.0, 28.0),
        19.0,
        GOLD,
        Justify::Center,
    );
    spawn_aux_text(
        parent,
        &format!("Delete '{}' (slot {})?", name, index),
        spec::CrystalRect::new(366.0, 350.0, 292.0, 25.0),
        14.0,
        CREAM,
        Justify::Center,
    );
    if let Some(notice) = &model.notice {
        spawn_aux_notice(
            parent,
            &notice.message,
            notice.kind,
            spec::CrystalRect::new(366.0, 374.0, 292.0, 20.0),
        );
    }
    let confirm_spec = CrystalButtonSpec::new(
        "Title",
        320,
        321,
        322,
        spec::CrystalRect::new(458.0, 398.0, 42.0, 42.0),
        48.0,
        48.0,
    );
    let cancel_spec = CrystalButtonSpec::new(
        "Title",
        329,
        330,
        331,
        spec::CrystalRect::new(514.0, 407.0, 100.0, 25.0),
        100.0,
        25.0,
    );
    spawn_crystal_image_button(
        parent,
        asset_server,
        confirm_spec,
        CrystalButtonAssetSet::from_spec(confirm_spec),
        NativeShellButton::ConfirmDelete,
        aux_focus.delete_confirm == DeleteConfirmFocus::Confirm,
        !model.delete_request_in_flight,
    );
    spawn_crystal_image_button(
        parent,
        asset_server,
        cancel_spec,
        CrystalButtonAssetSet::from_spec(cancel_spec),
        NativeShellButton::CancelDelete,
        aux_focus.delete_confirm == DeleteConfirmFocus::Cancel,
        !model.delete_request_in_flight,
    );
    if model.delete_request_in_flight {
        spawn_aux_text(
            parent,
            "Deleting...",
            spec::CrystalRect::new(366.0, 454.0, 292.0, 20.0),
            12.0,
            GOLD,
            Justify::Center,
        );
    }
}

fn render_connection_lost(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    model: &NativeShellModel,
    aux_focus: NativeShellAuxFocus,
) {
    spawn_native_image(
        parent,
        asset_server,
        "original-ui/Prguse/360.png",
        CONNECTION_LOST_PANEL,
    );
    spawn_aux_text(
        parent,
        "Connection Lost",
        spec::CrystalRect::new(319.0, 316.0, 390.0, 24.0),
        18.0,
        GOLD,
        Justify::Center,
    );
    spawn_aux_text(
        parent,
        "Press Enter, Escape, or Retry to reconnect.",
        spec::CrystalRect::new(319.0, 348.0, 390.0, 30.0),
        12.0,
        CREAM,
        Justify::Center,
    );
    if let Some(notice) = &model.notice {
        let notice_summary = connection_notice_summary(&notice.message);
        spawn_aux_notice(
            parent,
            &notice_summary,
            notice.kind,
            spec::CrystalRect::new(319.0, 388.0, 390.0, 34.0),
        );
    }
    let retry_spec = connection_lost_retry_spec();
    spawn_crystal_image_button(
        parent,
        asset_server,
        retry_spec,
        CrystalButtonAssetSet::from_spec(retry_spec),
        NativeShellButton::Retry,
        aux_focus.connection_retry,
        true,
    );
}

const fn connection_lost_retry_spec() -> CrystalButtonSpec {
    CrystalButtonSpec::new(
        "Title",
        200,
        201,
        202,
        spec::CrystalRect::new(644.0, 446.0, 76.0, 25.0),
        76.0,
        25.0,
    )
}

fn connection_notice_summary(message: &str) -> String {
    if let Some(start) = message.rfind("(os error ") {
        if let Some(relative_end) = message[start..].find(')') {
            let end = start + relative_end + 1;
            return format!("Cannot reach the local Gateway.\n{}", &message[start..end]);
        }
    }

    const MAX_NOTICE_CHARS: usize = 72;
    let mut summary = message.split_whitespace().collect::<Vec<_>>().join(" ");
    if summary.chars().count() > MAX_NOTICE_CHARS {
        summary = summary.chars().take(MAX_NOTICE_CHARS - 1).collect();
        summary.push('…');
    }
    summary
}

fn character_class_index(class_name: &str) -> u16 {
    CLASSES
        .iter()
        .position(|candidate| candidate.eq_ignore_ascii_case(class_name))
        .unwrap_or(0) as u16
}

fn character_gender_index(gender_name: &str) -> u16 {
    GENDERS
        .iter()
        .position(|candidate| candidate.eq_ignore_ascii_case(gender_name))
        .unwrap_or(0) as u16
}

fn character_description(class_name: &str) -> &'static str {
    match character_class_index(class_name) {
        0 => "Warriors are resilient frontline fighters.\nThey favor close combat and heavy weapons.",
        1 => "Wizards command powerful elemental magic.\nThey favor ranged spells and careful positioning.",
        2 => "Taoists support allies and master spiritual arts.\nThey balance healing, buffs, and combat.",
        _ => "",
    }
}

fn spawn_native_image(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    path: impl Into<String>,
    rect: spec::CrystalRect,
) {
    let path = path.into();
    parent.spawn((
        absolute_node(rect),
        ImageNode {
            image: asset_server.load(path),
            image_mode: NodeImageMode::Stretch,
            ..default()
        },
    ));
}

fn spawn_auxiliary_panel(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    rect: spec::CrystalRect,
) {
    parent.spawn((
        absolute_node(rect),
        BackgroundColor(PANEL_BG),
        ImageNode {
            image: asset_server.load("original-ui/Prguse/1084.png"),
            image_mode: NodeImageMode::Stretch,
            ..default()
        },
    ));
    parent.spawn((
        absolute_node(rect),
        BackgroundColor(Color::srgba(0.04, 0.025, 0.015, 0.42)),
    ));
}

fn absolute_node(rect: spec::CrystalRect) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(rect.left),
        top: Val::Px(rect.top),
        width: Val::Px(rect.width),
        height: Val::Px(rect.height),
        ..default()
    }
}

fn spawn_aux_text(
    parent: &mut ChildSpawnerCommands,
    value: &str,
    rect: spec::CrystalRect,
    size: f32,
    color: Color,
    justify: Justify,
) {
    let mut node = absolute_node(rect);
    node.overflow = Overflow::clip();
    parent.spawn((
        node,
        Text::new(value.to_owned()),
        body_font(size),
        TextColor(color),
        TextLayout::justify(justify),
        TextShadow {
            offset: Vec2::splat(1.0),
            color: Color::BLACK,
        },
    ));
}

fn spawn_aux_notice(
    parent: &mut ChildSpawnerCommands,
    message: &str,
    kind: crate::native_shell::ShellNoticeKind,
    rect: spec::CrystalRect,
) {
    let color = match kind {
        crate::native_shell::ShellNoticeKind::Info => CREAM,
        crate::native_shell::ShellNoticeKind::Warn => GOLD,
        crate::native_shell::ShellNoticeKind::Error => Color::srgb(0.95, 0.38, 0.30),
    };
    spawn_aux_text(parent, message, rect, 12.0, color, Justify::Center);
}

fn spawn_aux_field(
    parent: &mut ChildSpawnerCommands,
    rect: spec::CrystalRect,
    value: &str,
    focused: bool,
    enabled: bool,
    field: NativeShellField,
) {
    let mut entity = parent.spawn((
        absolute_node(rect),
        BackgroundColor(if focused {
            BUTTON_HIGHLIGHT
        } else {
            Color::BLACK
        }),
        field,
    ));
    if enabled {
        entity.insert(Button);
    }
    entity.with_children(|input| {
        input.spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                padding: UiRect::horizontal(Val::Px(8.0)),
                align_items: AlignItems::Center,
                ..default()
            },
            Text::new(value.to_owned()),
            body_font(14.0),
            TextColor(CREAM),
        ));
    });
}

fn spawn_crystal_name_field(
    parent: &mut ChildSpawnerCommands,
    rect: spec::CrystalRect,
    value: &str,
    _focused: bool,
    enabled: bool,
    field: NativeShellField,
) {
    let mut entity = parent.spawn((
        absolute_node(rect),
        // The Prguse/73 frame supplies the gold border.  Keep the input's
        // interior black so focus does not paint over Crystal's border art.
        BackgroundColor(Color::BLACK),
        field,
    ));
    if enabled {
        entity.insert(Button);
    }
    entity.with_children(|input| {
        input.spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                padding: UiRect::horizontal(Val::Px(2.0)),
                align_items: AlignItems::Center,
                ..default()
            },
            Text::new(value.to_owned()),
            body_font(14.0),
            TextColor(CREAM),
        ));
    });
}

fn spawn_aux_text_button(
    parent: &mut ChildSpawnerCommands,
    rect: spec::CrystalRect,
    label: String,
    action: NativeShellButton,
    enabled: bool,
    focused: bool,
) {
    let mut entity = parent.spawn((
        absolute_node(rect),
        BackgroundColor(if focused { BUTTON_HIGHLIGHT } else { BUTTON_BG }),
        action,
    ));
    if enabled {
        entity.insert(Button);
    }
    entity.with_children(|button| {
        button.spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                display: Display::Flex,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            Text::new(label),
            body_font(12.0),
            TextColor(CREAM),
            TextLayout::justify(Justify::Center),
        ));
    });
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
    #[test]
    fn door_waits_for_all_frames_then_updates_the_real_background_handle() {
        use super::*;
        let mut images = Assets::<Image>::default();
        let frames = (0..19)
            .map(|_| images.add(Image::default()))
            .collect::<Vec<_>>();
        let missing = images.remove(frames[18].id()).unwrap();
        let mut app = App::new();
        app.insert_resource(images);
        app.insert_resource(NativeLoginDoorFrames(frames.clone()));
        app.insert_resource(Time::<()>::default());
        let mut model = NativeShellModel::default();
        model.screen = NativeShellScreen::OpeningLogin;
        app.insert_resource(model);
        let background = app
            .world_mut()
            .spawn((NativeLoginDoorBackground, ImageNode::default()))
            .id();
        app.add_systems(Update, animate_login_door);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(100));
        app.update();
        assert_eq!(
            app.world()
                .resource::<NativeShellModel>()
                .login_opening_elapsed,
            std::time::Duration::ZERO
        );
        assert_eq!(
            app.world().get::<ImageNode>(background).unwrap().image,
            frames[0]
        );
        app.world_mut()
            .resource_mut::<Assets<Image>>()
            .insert(frames[18].id(), missing)
            .unwrap();
        app.update();
        assert_eq!(
            app.world().get::<ImageNode>(background).unwrap().image,
            frames[2]
        );
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(1700));
        app.update();
        assert_eq!(
            app.world().resource::<NativeShellModel>().screen,
            NativeShellScreen::CharacterSelect
        );
    }

    use super::*;
    use crate::native_shell::{CharacterSummary, NativeGatewayEvent};

    fn keyboard_event(
        key_code: KeyCode,
        logical_key: Key,
        state: ButtonState,
        text: Option<&str>,
    ) -> KeyboardInput {
        KeyboardInput {
            key_code,
            logical_key,
            state,
            text: text.map(Into::into),
            repeat: false,
            window: Entity::PLACEHOLDER,
        }
    }

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
    fn rapid_keyboard_batch_consumes_text_and_logical_character_fallbacks_in_order() {
        let characters = [
            (KeyCode::KeyV, "v", Some("v")),
            (KeyCode::KeyI, "i", None),
            (KeyCode::KeyS, "s", Some("s")),
            (KeyCode::KeyU, "u", None),
            (KeyCode::KeyA, "a", Some("a")),
            (KeyCode::KeyL, "l", None),
            (KeyCode::KeyQ, "q", Some("q")),
            (KeyCode::KeyA, "a", None),
            (KeyCode::Digit1, "1", Some("1")),
        ];
        let events = characters
            .into_iter()
            .map(|(key_code, logical, text)| {
                keyboard_event(
                    key_code,
                    Key::Character(logical.into()),
                    ButtonState::Pressed,
                    text,
                )
            })
            .collect::<Vec<_>>();
        let mut modifiers = NativeShellTextModifiers::default();

        assert_eq!(
            collect_typed_text(events.iter(), &mut modifiers),
            "visualqa1"
        );
    }

    #[test]
    fn single_event_can_carry_multiple_composed_characters() {
        let event = keyboard_event(
            KeyCode::KeyV,
            Key::Character("vqa0822r".into()),
            ButtonState::Pressed,
            Some("vqa0822r"),
        );
        let mut modifiers = NativeShellTextModifiers::default();

        assert_eq!(collect_typed_text([&event], &mut modifiers), "vqa0822r");
    }

    #[test]
    fn empty_composed_text_uses_logical_character_fallback() {
        let event = keyboard_event(
            KeyCode::KeyQ,
            Key::Character("q".into()),
            ButtonState::Pressed,
            Some(""),
        );
        let mut modifiers = NativeShellTextModifiers::default();

        assert_eq!(collect_typed_text([&event], &mut modifiers), "q");
    }

    #[test]
    fn logical_character_fallback_does_not_turn_control_shortcuts_into_text() {
        let events = [
            keyboard_event(
                KeyCode::ControlLeft,
                Key::Control,
                ButtonState::Pressed,
                None,
            ),
            keyboard_event(
                KeyCode::KeyV,
                Key::Character("v".into()),
                ButtonState::Pressed,
                None,
            ),
            keyboard_event(
                KeyCode::ControlLeft,
                Key::Control,
                ButtonState::Released,
                None,
            ),
            keyboard_event(
                KeyCode::KeyA,
                Key::Character("a".into()),
                ButtonState::Pressed,
                None,
            ),
        ];
        let mut modifiers = NativeShellTextModifiers::default();

        assert_eq!(collect_typed_text(events.iter(), &mut modifiers), "a");
    }

    #[test]
    fn composed_text_does_not_turn_control_shortcuts_into_text() {
        let events = [
            keyboard_event(
                KeyCode::ControlLeft,
                Key::Control,
                ButtonState::Pressed,
                None,
            ),
            keyboard_event(
                KeyCode::KeyA,
                Key::Character("a".into()),
                ButtonState::Pressed,
                Some("a"),
            ),
            keyboard_event(
                KeyCode::ControlLeft,
                Key::Control,
                ButtonState::Released,
                None,
            ),
            keyboard_event(
                KeyCode::KeyA,
                Key::Character("a".into()),
                ButtonState::Pressed,
                Some("a"),
            ),
        ];
        let mut modifiers = NativeShellTextModifiers::default();

        assert_eq!(collect_typed_text(events.iter(), &mut modifiers), "a");
    }

    #[test]
    fn login_system_applies_an_entire_rapid_character_batch_in_one_update() {
        let mut app = App::new();
        app.insert_resource(ButtonInput::<KeyCode>::default())
            .insert_resource(NativeShellModel {
                screen: NativeShellScreen::Login,
                ..Default::default()
            })
            .init_resource::<NativeUiIntentQueue>()
            .init_resource::<NativeShellAuxFocus>()
            .init_resource::<NativeShellTextModifiers>()
            .add_message::<KeyboardInput>()
            .add_systems(Update, shell_keyboard_input);

        for (key_code, character, with_text) in [
            (KeyCode::KeyV, "v", true),
            (KeyCode::KeyI, "i", false),
            (KeyCode::KeyS, "s", true),
            (KeyCode::KeyU, "u", false),
            (KeyCode::KeyA, "a", true),
            (KeyCode::KeyL, "l", false),
            (KeyCode::KeyQ, "q", true),
            (KeyCode::KeyA, "a", false),
            (KeyCode::Digit1, "1", true),
        ] {
            app.world_mut().write_message(keyboard_event(
                key_code,
                Key::Character(character.into()),
                ButtonState::Pressed,
                with_text.then_some(character),
            ));
        }

        app.update();
        assert_eq!(
            app.world().resource::<NativeShellModel>().login.account,
            "visualqa1"
        );
    }

    #[test]
    fn login_system_honors_backspace_and_delete_repeat_messages() {
        let mut shell = NativeShellModel {
            screen: NativeShellScreen::Login,
            ..Default::default()
        };
        shell.login.account = "visualqa1".to_owned();
        shell.login.password = "secret".to_owned();
        let mut app = App::new();
        app.insert_resource(ButtonInput::<KeyCode>::default())
            .insert_resource(shell)
            .init_resource::<NativeUiIntentQueue>()
            .init_resource::<NativeShellAuxFocus>()
            .init_resource::<NativeShellTextModifiers>()
            .add_message::<KeyboardInput>()
            .add_systems(Update, shell_keyboard_input);

        for repeat in [false, true, true] {
            let mut event = keyboard_event(
                KeyCode::Backspace,
                Key::Backspace,
                ButtonState::Pressed,
                None,
            );
            event.repeat = repeat;
            app.world_mut().write_message(event);
        }
        app.update();
        assert_eq!(
            app.world().resource::<NativeShellModel>().login.account,
            "visual"
        );

        app.world_mut()
            .resource_mut::<NativeShellModel>()
            .login
            .focus = LoginFocus::Password;
        for repeat in [false, true] {
            let mut event =
                keyboard_event(KeyCode::Delete, Key::Delete, ButtonState::Pressed, None);
            event.repeat = repeat;
            app.world_mut().write_message(event);
        }
        app.update();
        assert_eq!(
            app.world().resource::<NativeShellModel>().login.password,
            "secr"
        );
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
        assert_eq!(
            cycle_delete_focus(DeleteConfirmFocus::Confirm, false),
            DeleteConfirmFocus::Cancel
        );
        assert_eq!(
            cycle_delete_focus(DeleteConfirmFocus::Cancel, true),
            DeleteConfirmFocus::Confirm
        );
    }

    #[test]
    fn new_character_uses_crystal_geometry_assets_and_supported_control_bounds() {
        assert_eq!(
            NEW_CHARACTER_FRAME,
            spec::CrystalRect::new(218.0, 154.0, 588.0, 460.0)
        );
        assert_eq!(
            NEW_CHARACTER_TITLE,
            spec::CrystalRect::new(424.0, 165.0, 187.0, 20.0)
        );
        assert_eq!(
            NEW_CHARACTER_NAME_FIELD,
            spec::CrystalRect::new(543.0, 422.0, 240.0, 20.0)
        );
        assert_eq!(NEW_CHARACTER_PREVIEW_ANCHOR, (338.0, 404.0));
        assert_eq!(
            NEW_CHARACTER_CREATE,
            spec::CrystalRect::new(378.0, 579.0, 100.0, 25.0)
        );
        assert_eq!(
            NEW_CHARACTER_CANCEL,
            spec::CrystalRect::new(643.0, 579.0, 100.0, 25.0)
        );
        assert_eq!(NEW_CHARACTER_CLASS_BUTTONS.len(), CLASSES.len());
        assert_eq!(NEW_CHARACTER_GENDER_BUTTONS.len(), GENDERS.len());
        assert!(NEW_CHARACTER_CLASS_BUTTONS
            .into_iter()
            .chain(NEW_CHARACTER_GENDER_BUTTONS)
            .all(|rect| rect.is_valid_hit_target()));
        assert!(NEW_CHARACTER_CLASS_BUTTONS
            .into_iter()
            .chain(NEW_CHARACTER_GENDER_BUTTONS)
            .all(|rect| NEW_CHARACTER_FRAME.contains(rect.left, rect.top)));
        assert_eq!(
            spec::CrystalFrameSpec::new("Prguse", 73, NEW_CHARACTER_FRAME).asset_path(),
            "original-ui/Prguse/73.png"
        );
        assert_eq!(
            spec::CrystalFrameSpec::new("Title", 20, NEW_CHARACTER_TITLE).asset_path(),
            "original-ui/Title/20.png"
        );
    }

    #[test]
    fn new_character_class_and_gender_choices_are_limited_to_existing_backend_flow() {
        assert_eq!(CLASSES, ["Warrior", "Wizard", "Taoist"]);
        assert_eq!(GENDERS, ["Male", "Female"]);
        assert!(!CLASSES.contains(&"Assassin"));
        assert!(!CLASSES.contains(&"Archer"));
        assert_eq!(character_description("Warrior").is_empty(), false);
    }

    #[test]
    fn auxiliary_fields_reject_invalid_name_and_password_characters() {
        let mut name = String::new();
        append_name_field(&mut name, ' ', MAX_NAME);
        append_name_field(&mut name, '\n', MAX_NAME);
        append_name_field(&mut name, '勇', MAX_NAME);
        assert_eq!(name, "勇");

        let mut password = String::new();
        append_alphanumeric_field(&mut password, 'a', MAX_CHANGE_PASSWORD);
        append_alphanumeric_field(&mut password, '@', MAX_CHANGE_PASSWORD);
        append_alphanumeric_field(&mut password, '7', MAX_CHANGE_PASSWORD);
        assert_eq!(password, "a7");
    }

    #[test]
    fn repeated_character_create_submission_is_not_queued_twice() {
        let mut model = NativeShellModel::default();
        model.screen = NativeShellScreen::CharacterCreate;
        model.character_create.name = "Hero".to_owned();
        let intent = NativeUiIntent::CreateCharacter {
            name: "Hero".to_owned(),
            class_name: "Warrior".to_owned(),
            gender_name: "Male".to_owned(),
        };
        let mut queue = NativeUiIntentQueue::default();

        assert!(apply_and_queue(&mut model, &mut queue, intent.clone()));
        assert!(!apply_and_queue(&mut model, &mut queue, intent));
        assert_eq!(queue.drain().count(), 1);
    }

    #[test]
    fn valid_registration_is_the_only_registration_intent_queued() {
        let mut model = NativeShellModel::default();
        model.screen = NativeShellScreen::Login;
        let mut queue = NativeUiIntentQueue::default();
        assert!(model.apply_ui_intent(NativeUiIntent::OpenRegistration));
        let intent = NativeUiIntent::SubmitRegistration {
            account_id: "hero".to_owned(),
            password: "secret".to_owned(),
            confirm_password: "secret".to_owned(),
            birth_date: "2000-01-01".to_owned(),
            birth_date_binary: parse_registration_birth_date("2000-01-01").unwrap(),
            user_name: "Hero".to_owned(),
            secret_question: "pet?".to_owned(),
            secret_answer: "cat".to_owned(),
            email_address: "hero@example.test".to_owned(),
        };
        assert!(apply_and_queue(&mut model, &mut queue, intent.clone()));
        assert_eq!(queue.drain().collect::<Vec<_>>(), vec![intent]);
        assert_eq!(model.screen, NativeShellScreen::Registration);
        assert!(model.register_request_in_flight);
    }

    #[test]
    fn draining_does_not_reopen_a_request_before_its_ack() {
        let mut model = NativeShellModel::default();
        model.screen = NativeShellScreen::CharacterCreate;
        model.character_create.name = "Hero".to_owned();
        let intent = NativeUiIntent::CreateCharacter {
            name: "Hero".to_owned(),
            class_name: "Warrior".to_owned(),
            gender_name: "Male".to_owned(),
        };
        let mut queue = NativeUiIntentQueue::default();

        assert!(apply_and_queue(&mut model, &mut queue, intent.clone()));
        assert_eq!(queue.drain().count(), 1);
        assert!(!apply_and_queue(&mut model, &mut queue, intent.clone()));
        assert!(queue.is_empty());
        assert!(model.create_character_request_in_flight);

        assert!(
            model.apply_gateway_event(NativeGatewayEvent::CharacterCreated {
                character: CharacterSummary::new(1, "Hero", 1, "Warrior", "Male"),
            })
        );
        assert!(!model.create_character_request_in_flight);
        assert!(model.apply_ui_intent(NativeUiIntent::OpenCharacterCreate));
        assert!(apply_and_queue(&mut model, &mut queue, intent));
        assert_eq!(queue.drain().count(), 1);
    }

    #[test]
    fn registration_failure_releases_submission_for_recovery() {
        let mut model = NativeShellModel::default();
        model.screen = NativeShellScreen::Registration;
        let mut queue = NativeUiIntentQueue::default();
        let intent = NativeUiIntent::SubmitRegistration {
            account_id: "hero".to_owned(),
            password: "secret".to_owned(),
            confirm_password: "secret".to_owned(),
            birth_date: String::new(),
            birth_date_binary: 0,
            user_name: String::new(),
            secret_question: String::new(),
            secret_answer: String::new(),
            email_address: String::new(),
        };
        assert!(apply_and_queue(&mut model, &mut queue, intent.clone()));
        queue.drain().for_each(drop);
        assert!(
            model.apply_gateway_event(NativeGatewayEvent::AccountCreationFailed {
                message: "already exists".to_owned(),
            })
        );
        assert!(!model.register_request_in_flight);
        assert!(apply_and_queue(&mut model, &mut queue, intent));
        assert_eq!(queue.drain().count(), 1);
    }

    #[test]
    fn duplicate_registration_submit_is_rejected_without_logging_password() {
        let mut model = NativeShellModel::default();
        model.screen = NativeShellScreen::Registration;
        let mut queue = NativeUiIntentQueue::default();
        let intent = NativeUiIntent::SubmitRegistration {
            account_id: "hero".to_owned(),
            password: "supersecret".to_owned(),
            confirm_password: "supersecret".to_owned(),
            birth_date: String::new(),
            birth_date_binary: 0,
            user_name: String::new(),
            secret_question: String::new(),
            secret_answer: String::new(),
            email_address: String::new(),
        };
        assert!(apply_and_queue(&mut model, &mut queue, intent.clone()));
        queue.drain().for_each(drop);
        assert!(!apply_and_queue(&mut model, &mut queue, intent.clone()));
        assert!(queue.is_empty());
        assert!(!format!("{model:?}").contains("supersecret"));
        assert!(!format!("{intent:?}").contains("supersecret"));
        assert!(!format!(
            "{:?}",
            NativeUiIntent::SubmitChangePassword {
                account_id: "hero".to_owned(),
                old_password: "super-secret".to_owned(),
                new_password: "new-secret".to_owned(),
                confirm_password: "new-secret".to_owned(),
            }
        )
        .contains("super-secret"));
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
    fn registration_submit_intent_preserves_every_supported_new_account_field() {
        let mut model = NativeShellModel::default();
        model.registration.account_id = "newhero".to_owned();
        model.registration.password = "Secret1".to_owned();
        model.registration.confirm_password = "Secret1".to_owned();
        model.registration.birth_date = "2001-02-03".to_owned();
        model.registration.user_name = "New Hero".to_owned();
        model.registration.secret_question = "first pet?".to_owned();
        model.registration.secret_answer = "cat".to_owned();
        model.registration.email_address = "hero@example.test".to_owned();

        assert_eq!(
            registration_submit_intent(&model),
            Ok(NativeUiIntent::SubmitRegistration {
                account_id: "newhero".to_owned(),
                password: "Secret1".to_owned(),
                confirm_password: "Secret1".to_owned(),
                birth_date: "2001-02-03".to_owned(),
                birth_date_binary: parse_registration_birth_date("2001-02-03").unwrap(),
                user_name: "New Hero".to_owned(),
                secret_question: "first pet?".to_owned(),
                secret_answer: "cat".to_owned(),
                email_address: "hero@example.test".to_owned(),
            })
        );
    }

    #[test]
    fn registration_renderer_enables_submit_only_for_a_valid_idle_form() {
        let mut model = NativeShellModel::default();
        assert!(!registration_submit_enabled(&model));

        model.registration.account_id = "newhero".to_owned();
        model.registration.password = "Secret1".to_owned();
        model.registration.confirm_password = "Secret1".to_owned();
        assert!(registration_submit_enabled(&model));

        model.register_request_in_flight = true;
        assert!(!registration_submit_enabled(&model));
    }

    #[test]
    fn registration_focus_order_covers_every_crystal_dialog_control() {
        use RegistrationFocus::*;

        let mut focus = AccountId;
        let mut visited = Vec::new();
        for _ in 0..10 {
            visited.push(focus);
            focus = cycle_registration_focus(focus, false);
        }
        assert_eq!(focus, AccountId);
        assert_eq!(
            visited,
            vec![
                AccountId,
                Password,
                ConfirmPassword,
                UserName,
                BirthDate,
                SecretQuestion,
                SecretAnswer,
                EmailAddress,
                SubmitButton,
                CancelButton,
            ]
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

    #[test]
    fn connection_notice_collapses_localized_socket_text_to_a_bounded_summary() {
        assert_eq!(
            connection_notice_summary(
                "gateway connect failed: IO error: 由于目标计算机积极拒绝，无法连接。 (os error 10061)"
            ),
            "Cannot reach the local Gateway.\n(os error 10061)"
        );
    }

    #[test]
    fn connection_notice_truncation_is_unicode_safe() {
        let source = "网关连接失败".repeat(20);
        let summary = connection_notice_summary(&source);
        assert_eq!(summary.chars().count(), 72);
        assert!(summary.ends_with('…'));
    }

    #[test]
    fn connection_lost_dialog_uses_crystal_message_box_and_ok_button_geometry() {
        assert_eq!(
            CONNECTION_LOST_PANEL,
            spec::CrystalRect::new(284.0, 289.0, 456.0, 190.0)
        );
        assert_eq!(
            connection_lost_retry_spec(),
            CrystalButtonSpec::new(
                "Title",
                200,
                201,
                202,
                spec::CrystalRect::new(644.0, 446.0, 76.0, 25.0),
                76.0,
                25.0,
            )
        );
    }
}
