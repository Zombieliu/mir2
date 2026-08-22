//! Platform-neutral shell model for Windows-native play flow.
//!
//! The module owns only UI state. Platform hosts translate native socket/gateway
//! traffic into `NativeGatewayEvent` and widgets into `NativeUiIntent`, then feed
//! both through deterministic reducers.

#![forbid(unsafe_code)]

use core::fmt;
use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};

use bevy::prelude::Resource;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeShellScreen {
    Connecting,
    Login,
    Authenticating,
    CharacterSelect,
    CharacterCreate,
    StartingGame,
    InGame,
    ConnectionLost,
    ChangePassword,
    SafeKey,
    DeleteConfirm { index: i32 },
}

impl Default for NativeShellScreen {
    fn default() -> Self {
        Self::Connecting
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginFocus {
    Account,
    Password,
    LoginButton,
    NewAccountButton,
}

#[derive(Clone, PartialEq, Eq)]
pub struct LoginForm {
    pub account: String,
    pub password: String,
    pub focus: LoginFocus,
}

impl Default for LoginForm {
    fn default() -> Self {
        Self {
            account: String::new(),
            password: String::new(),
            focus: LoginFocus::Account,
        }
    }
}

impl fmt::Debug for LoginForm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoginForm")
            .field("account", &self.account)
            .field("password", &"<redacted>")
            .field("focus", &self.focus)
            .finish()
    }
}

impl LoginForm {
    pub fn clear_password(&mut self) {
        self.password.clear();
    }

    pub fn has_account(&self) -> bool {
        !self.account.trim().is_empty()
    }

    pub fn has_password(&self) -> bool {
        !self.password.is_empty()
    }

    pub fn is_ready(&self) -> bool {
        self.has_account() && self.has_password()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterCreateFocus {
    Name,
    Class,
    Gender,
    CreateButton,
    CancelButton,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangePasswordFocus {
    AccountId,
    OldPassword,
    NewPassword,
    ConfirmPassword,
    SubmitButton,
    CancelButton,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ChangePasswordForm {
    pub account_id: String,
    pub old_password: String,
    pub new_password: String,
    pub confirm_password: String,
    pub focus: ChangePasswordFocus,
}

impl Default for ChangePasswordForm {
    fn default() -> Self {
        Self {
            account_id: String::new(),
            old_password: String::new(),
            new_password: String::new(),
            confirm_password: String::new(),
            focus: ChangePasswordFocus::AccountId,
        }
    }
}

impl fmt::Debug for ChangePasswordForm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChangePasswordForm")
            .field("account_id", &self.account_id)
            .field("old_password", &"<redacted>")
            .field("new_password", &"<redacted>")
            .field("confirm_password", &"<redacted>")
            .field("focus", &self.focus)
            .finish()
    }
}

impl ChangePasswordForm {
    pub fn for_account(account_id: impl Into<String>) -> Self {
        Self {
            account_id: account_id.into(),
            ..Self::default()
        }
    }
}

const MIN_ACCOUNT_ID_LENGTH: usize = 3;
const MAX_ACCOUNT_ID_LENGTH: usize = 15;
const MIN_PASSWORD_LENGTH: usize = 5;
const MAX_PASSWORD_LENGTH: usize = 15;
const SAFE_KEY_DEFAULT_SEED: u64 = 0x4D49_5232_5341_4645;
const SAFE_KEY_ALPHABET: [char; 36] = [
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S',
    'T', 'U', 'V', 'W', 'X', 'Y', 'Z', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafeKeyState {
    pub keys: Vec<char>,
    seed: u64,
}

impl Default for SafeKeyState {
    fn default() -> Self {
        Self::from_seed(SAFE_KEY_DEFAULT_SEED)
    }
}

impl SafeKeyState {
    pub fn from_seed(seed: u64) -> Self {
        Self {
            keys: safe_key_permutation(seed),
            seed,
        }
    }

    fn reshuffle(&mut self) {
        self.seed = next_safe_key_seed(self.seed);
        self.keys = safe_key_permutation(self.seed);
    }
}

pub fn safe_key_permutation(seed: u64) -> Vec<char> {
    let mut keys = SAFE_KEY_ALPHABET.to_vec();
    let mut state = if seed == 0 {
        SAFE_KEY_DEFAULT_SEED
    } else {
        seed
    };

    for i in (1..keys.len()).rev() {
        state = splitmix64(state);
        let j = (state % (i as u64 + 1)) as usize;
        keys.swap(i, j);
    }
    keys
}

fn splitmix64(mut state: u64) -> u64 {
    state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn next_safe_key_seed(seed: u64) -> u64 {
    splitmix64(seed)
}

fn random_safe_key_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(SAFE_KEY_DEFAULT_SEED)
}

fn valid_alphanumeric(value: &str, min: usize, max: usize) -> bool {
    let length = value.chars().count();
    (min..=max).contains(&length)
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
}

pub fn validate_change_password_fields(
    account_id: &str,
    old_password: &str,
    new_password: &str,
    confirm_password: &str,
) -> Result<(), &'static str> {
    if !valid_alphanumeric(account_id, MIN_ACCOUNT_ID_LENGTH, MAX_ACCOUNT_ID_LENGTH) {
        return Err("account ID must be 3-15 alphanumeric characters");
    }
    if !valid_alphanumeric(old_password, MIN_PASSWORD_LENGTH, MAX_PASSWORD_LENGTH) {
        return Err("current password must be 5-15 alphanumeric characters");
    }
    if !valid_alphanumeric(new_password, MIN_PASSWORD_LENGTH, MAX_PASSWORD_LENGTH) {
        return Err("new password must be 5-15 alphanumeric characters");
    }
    if new_password != confirm_password {
        return Err("new password confirmation does not match");
    }
    if !valid_alphanumeric(confirm_password, MIN_PASSWORD_LENGTH, MAX_PASSWORD_LENGTH) {
        return Err("new password confirmation must be 5-15 alphanumeric characters");
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterCreateForm {
    pub name: String,
    pub class_name: String,
    pub gender_name: String,
    pub focus: CharacterCreateFocus,
}

impl Default for CharacterCreateForm {
    fn default() -> Self {
        Self {
            name: String::new(),
            class_name: "Warrior".to_owned(),
            gender_name: "Male".to_owned(),
            focus: CharacterCreateFocus::Name,
        }
    }
}

impl CharacterCreateForm {
    pub fn is_ready(&self) -> bool {
        !self.name.trim().is_empty()
            && !self.class_name.trim().is_empty()
            && !self.gender_name.trim().is_empty()
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct CharacterSummary {
    pub index: i32,
    pub name: String,
    pub level: u16,
    pub class_name: String,
    pub gender_name: String,
}

impl fmt::Debug for CharacterSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CharacterSummary")
            .field("index", &self.index)
            .field("name", &self.name)
            .field("level", &self.level)
            .field("class_name", &self.class_name)
            .field("gender_name", &self.gender_name)
            .finish()
    }
}

impl CharacterSummary {
    pub fn new(index: i32, name: &str, level: u16, class_name: &str, gender_name: &str) -> Self {
        Self {
            index,
            name: name.to_string(),
            level,
            class_name: class_name.to_owned(),
            gender_name: gender_name.to_owned(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellNoticeKind {
    Info,
    Warn,
    Error,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ShellNotice {
    pub kind: ShellNoticeKind,
    pub message: String,
}

impl fmt::Debug for ShellNotice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShellNotice")
            .field("kind", &self.kind)
            .field("message", &self.message)
            .finish()
    }
}

impl ShellNotice {
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            kind: ShellNoticeKind::Info,
            message: message.into(),
        }
    }

    pub fn warn(message: impl Into<String>) -> Self {
        Self {
            kind: ShellNoticeKind::Warn,
            message: message.into(),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            kind: ShellNoticeKind::Error,
            message: message.into(),
        }
    }
}

#[derive(Clone, Default, PartialEq, Eq, Resource)]
pub struct NativeShellModel {
    pub screen: NativeShellScreen,
    pub login: LoginForm,
    pub login_request_in_flight: bool,
    pub register_request_in_flight: bool,
    pub character_create: CharacterCreateForm,
    pub create_character_request_in_flight: bool,
    pub change_password: ChangePasswordForm,
    pub change_password_request_in_flight: bool,
    pub change_password_command_sent: bool,
    pub safe_key: SafeKeyState,
    pub delete_confirm_index: Option<i32>,
    pub delete_request_in_flight: bool,
    pub delete_command_sent: bool,
    pub start_game_request_in_flight: bool,
    pub retry_request_in_flight: bool,
    pub logout_request_in_flight: bool,
    pub notice: Option<ShellNotice>,
    pub characters: Vec<CharacterSummary>,
    pub selected_character_index: Option<i32>,
    pub active_character: Option<CharacterSummary>,
    pub last_account: Option<String>,
}

impl fmt::Debug for NativeShellModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeShellModel")
            .field("screen", &self.screen)
            .field("login", &self.login)
            .field("login_request_in_flight", &self.login_request_in_flight)
            .field(
                "register_request_in_flight",
                &self.register_request_in_flight,
            )
            .field("character_create", &self.character_create)
            .field(
                "create_character_request_in_flight",
                &self.create_character_request_in_flight,
            )
            .field("change_password", &self.change_password)
            .field(
                "change_password_request_in_flight",
                &self.change_password_request_in_flight,
            )
            .field("safe_key", &self.safe_key)
            .field(
                "start_game_request_in_flight",
                &self.start_game_request_in_flight,
            )
            .field("retry_request_in_flight", &self.retry_request_in_flight)
            .field("logout_request_in_flight", &self.logout_request_in_flight)
            .field("notice", &self.notice)
            .field("characters", &self.characters)
            .field("selected_character_index", &self.selected_character_index)
            .field("active_character", &self.active_character)
            .field("last_account", &self.last_account)
            .finish()
    }
}

impl NativeShellModel {
    fn clear_session_payload(&mut self) {
        self.characters.clear();
        self.selected_character_index = None;
        self.active_character = None;
        self.change_password = ChangePasswordForm::default();
        self.change_password_request_in_flight = false;
        self.change_password_command_sent = false;
        self.safe_key = SafeKeyState::default();
        self.delete_confirm_index = None;
        self.delete_request_in_flight = false;
        self.delete_command_sent = false;
        self.login_request_in_flight = false;
        self.register_request_in_flight = false;
        self.create_character_request_in_flight = false;
        self.start_game_request_in_flight = false;
        self.retry_request_in_flight = false;
        self.logout_request_in_flight = false;
        self.notice = None;
    }

    fn set_error(&mut self, message: impl Into<String>) {
        self.notice = Some(ShellNotice::error(message));
    }

    fn set_info(&mut self, message: impl Into<String>) {
        self.notice = Some(ShellNotice::info(message));
    }

    pub fn has_character_index(&self, index: i32) -> bool {
        self.characters
            .iter()
            .any(|candidate| candidate.index == index)
    }

    pub fn delete_command_pending(&self) -> Option<i32> {
        (self.delete_request_in_flight && !self.delete_command_sent)
            .then_some(self.delete_confirm_index)
            .flatten()
    }

    pub fn mark_delete_command_sent(&mut self) {
        if self.delete_request_in_flight {
            self.delete_command_sent = true;
        }
    }

    pub fn change_password_command_pending(&self) -> bool {
        self.change_password_request_in_flight && !self.change_password_command_sent
    }

    pub fn mark_change_password_command_sent(&mut self) {
        if self.change_password_request_in_flight {
            self.change_password_command_sent = true;
        }
    }
}

/// User intents from Bevy UI widgets.
#[derive(Clone, PartialEq, Eq)]
pub enum NativeUiIntent {
    Login,
    RegisterAccount,
    OpenChangePassword,
    SubmitChangePassword {
        account_id: String,
        old_password: String,
        new_password: String,
        confirm_password: String,
    },
    CancelChangePassword,
    OpenSafeKey,
    CloseSafeKey,
    SafeKeyFocusAccount,
    SafeKeyFocusPassword,
    SafeKeyPress {
        key: char,
    },
    SafeKeyDelete,
    SafeKeyRandom,
    SafeKeyEnter,
    OpenCharacterCreate,
    CreateCharacter {
        name: String,
        class_name: String,
        gender_name: String,
    },
    CancelCharacterCreate,
    DeleteCharacter {
        character_index: i32,
    },
    ConfirmDeleteCharacter,
    CancelDeleteCharacter,
    SelectCharacter {
        character_index: i32,
    },
    StartGame,
    Retry,
    Logout,
}

impl fmt::Debug for NativeUiIntent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SubmitChangePassword { account_id, .. } => f
                .debug_struct("SubmitChangePassword")
                .field("account_id", account_id)
                .field("old_password", &"<redacted>")
                .field("new_password", &"<redacted>")
                .field("confirm_password", &"<redacted>")
                .finish(),
            Self::Login
            | Self::RegisterAccount
            | Self::OpenChangePassword
            | Self::CancelChangePassword
            | Self::OpenSafeKey
            | Self::CloseSafeKey
            | Self::SafeKeyFocusAccount
            | Self::SafeKeyFocusPassword
            | Self::SafeKeyDelete
            | Self::SafeKeyRandom
            | Self::SafeKeyEnter
            | Self::OpenCharacterCreate
            | Self::CancelCharacterCreate
            | Self::ConfirmDeleteCharacter
            | Self::CancelDeleteCharacter
            | Self::StartGame
            | Self::Retry
            | Self::Logout => write!(
                f,
                "{}",
                match self {
                    Self::Login => "Login",
                    Self::RegisterAccount => "RegisterAccount",
                    Self::OpenChangePassword => "OpenChangePassword",
                    Self::CancelChangePassword => "CancelChangePassword",
                    Self::OpenSafeKey => "OpenSafeKey",
                    Self::CloseSafeKey => "CloseSafeKey",
                    Self::SafeKeyFocusAccount => "SafeKeyFocusAccount",
                    Self::SafeKeyFocusPassword => "SafeKeyFocusPassword",
                    Self::SafeKeyDelete => "SafeKeyDelete",
                    Self::SafeKeyRandom => "SafeKeyRandom",
                    Self::SafeKeyEnter => "SafeKeyEnter",
                    Self::OpenCharacterCreate => "OpenCharacterCreate",
                    Self::CancelCharacterCreate => "CancelCharacterCreate",
                    Self::ConfirmDeleteCharacter => "ConfirmDeleteCharacter",
                    Self::CancelDeleteCharacter => "CancelDeleteCharacter",
                    Self::StartGame => "StartGame",
                    Self::Retry => "Retry",
                    Self::Logout => "Logout",
                    _ => unreachable!("covered by the outer match"),
                }
            ),
            Self::SafeKeyPress { key } => f.debug_struct("SafeKeyPress").field("key", key).finish(),
            Self::CreateCharacter {
                name,
                class_name,
                gender_name,
            } => f
                .debug_struct("CreateCharacter")
                .field("name", name)
                .field("class_name", class_name)
                .field("gender_name", gender_name)
                .finish(),
            Self::DeleteCharacter { character_index }
            | Self::SelectCharacter { character_index } => f
                .debug_struct(match self {
                    Self::DeleteCharacter { .. } => "DeleteCharacter",
                    Self::SelectCharacter { .. } => "SelectCharacter",
                    _ => unreachable!("covered by the outer match"),
                })
                .field("character_index", character_index)
                .finish(),
        }
    }
}

/// Gateway callbacks for shell transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeGatewayEvent {
    Connected,
    AccountCreated,
    AccountCreationFailed {
        message: String,
    },
    LoginSuccess {
        account: String,
        characters: Vec<CharacterSummary>,
    },
    LoginFailure {
        message: String,
    },
    ChangePasswordResult {
        result: i32,
    },
    ChangePasswordBanned {
        reason: String,
        expiry: Option<String>,
    },
    CharacterCreated {
        character: CharacterSummary,
    },
    CharacterDeleted {
        character_index: i32,
    },
    StartGameAck {
        accepted: bool,
        reason: Option<String>,
    },
    PlayerBootstrapped {
        character: CharacterSummary,
    },
    OperationFailure {
        message: String,
    },
    LoggedOut {
        characters: Vec<CharacterSummary>,
    },
    Disconnect {
        reason: Option<String>,
    },
}

/// Platform-neutral handoff between Bevy widgets and the platform Gateway
/// adapter. UI systems enqueue intents; the Windows host drains and forwards
/// them without giving presentation code direct socket access.
#[derive(Debug, Default, Resource)]
pub struct NativeUiIntentQueue {
    pending: VecDeque<NativeUiIntent>,
}

impl NativeUiIntentQueue {
    pub fn push(&mut self, intent: NativeUiIntent) {
        self.pending.push_back(intent);
    }

    pub fn drain(&mut self) -> impl Iterator<Item = NativeUiIntent> + '_ {
        self.pending.drain(..)
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

impl NativeShellModel {
    fn begin_login(&mut self) -> bool {
        if self.login_request_in_flight {
            self.set_error("login request is still pending");
            return false;
        }
        if !self.login.is_ready() {
            self.set_error("account and password are required");
            return false;
        }
        self.screen = NativeShellScreen::Authenticating;
        self.login_request_in_flight = true;
        self.notice = None;
        true
    }

    pub fn apply_ui_intent(&mut self, intent: NativeUiIntent) -> bool {
        match (self.screen, intent) {
            (NativeShellScreen::Login, NativeUiIntent::Login) => self.begin_login(),
            (NativeShellScreen::Login, NativeUiIntent::RegisterAccount) => {
                if self.register_request_in_flight {
                    self.set_error("account creation request is still pending");
                    return false;
                }
                if !self.login.is_ready() {
                    self.set_error("account and password are required");
                    return false;
                }
                self.register_request_in_flight = true;
                self.set_info("account creation requested");
                true
            }
            (
                NativeShellScreen::CharacterSelect,
                NativeUiIntent::SelectCharacter { character_index },
            ) => {
                if self.has_character_index(character_index) {
                    self.selected_character_index = Some(character_index);
                    self.notice = None;
                    true
                } else {
                    self.set_error("invalid character index");
                    false
                }
            }
            (NativeShellScreen::CharacterSelect, NativeUiIntent::OpenCharacterCreate) => {
                self.character_create.reset();
                self.screen = NativeShellScreen::CharacterCreate;
                self.notice = None;
                true
            }
            (
                NativeShellScreen::CharacterCreate,
                NativeUiIntent::CreateCharacter {
                    name,
                    class_name,
                    gender_name,
                },
            ) => {
                if name.trim().is_empty() {
                    self.set_error("character name is required");
                    false
                } else if self.create_character_request_in_flight {
                    self.set_error("character creation request is still pending");
                    false
                } else {
                    self.create_character_request_in_flight = true;
                    self.set_info(format!(
                        "create character requested name={name} class={class_name} gender={gender_name}",
                    ));
                    true
                }
            }
            (NativeShellScreen::CharacterCreate, NativeUiIntent::CancelCharacterCreate) => {
                self.character_create.reset();
                self.screen = NativeShellScreen::CharacterSelect;
                self.notice = None;
                true
            }
            (NativeShellScreen::CharacterSelect, NativeUiIntent::StartGame) => {
                if self.start_game_request_in_flight {
                    self.set_error("start game request is still pending");
                    return false;
                }
                match self.selected_character_index {
                    Some(index) if self.has_character_index(index) => {
                        self.screen = NativeShellScreen::StartingGame;
                        self.start_game_request_in_flight = true;
                        self.notice = None;
                        true
                    }
                    _ => {
                        self.set_error("a valid character must be selected");
                        false
                    }
                }
            }
            (NativeShellScreen::Login, NativeUiIntent::OpenChangePassword) => {
                self.change_password = ChangePasswordForm::for_account(self.login.account.trim());
                self.change_password_request_in_flight = false;
                self.change_password_command_sent = false;
                self.screen = NativeShellScreen::ChangePassword;
                self.notice = None;
                true
            }
            (NativeShellScreen::ChangePassword, NativeUiIntent::CancelChangePassword) => {
                if self.change_password_request_in_flight {
                    self.set_error("change password request is still pending");
                    return false;
                }
                self.change_password = ChangePasswordForm::default();
                self.change_password_request_in_flight = false;
                self.change_password_command_sent = false;
                self.screen = NativeShellScreen::Login;
                self.notice = None;
                true
            }
            (
                NativeShellScreen::ChangePassword,
                NativeUiIntent::SubmitChangePassword {
                    account_id,
                    old_password,
                    new_password,
                    confirm_password,
                },
            ) => {
                self.change_password.account_id = account_id;
                self.change_password.old_password = old_password;
                self.change_password.new_password = new_password;
                self.change_password.confirm_password = confirm_password;

                if self.change_password_request_in_flight {
                    self.set_error("change password request is still pending");
                    return false;
                }

                if let Err(message) = validate_change_password_fields(
                    &self.change_password.account_id,
                    &self.change_password.old_password,
                    &self.change_password.new_password,
                    &self.change_password.confirm_password,
                ) {
                    self.set_error(message);
                    return false;
                }

                self.change_password_request_in_flight = true;
                self.change_password_command_sent = false;
                self.set_info("change password requested");
                true
            }
            (NativeShellScreen::Login, NativeUiIntent::OpenSafeKey) => {
                if !matches!(self.login.focus, LoginFocus::Account | LoginFocus::Password) {
                    self.login.focus = LoginFocus::Account;
                }
                self.safe_key = SafeKeyState::from_seed(random_safe_key_seed());
                self.screen = NativeShellScreen::SafeKey;
                self.notice = None;
                true
            }
            (NativeShellScreen::SafeKey, NativeUiIntent::CloseSafeKey) => {
                self.screen = NativeShellScreen::Login;
                self.notice = None;
                true
            }
            (NativeShellScreen::SafeKey, NativeUiIntent::SafeKeyFocusAccount) => {
                self.login.focus = LoginFocus::Account;
                self.notice = None;
                true
            }
            (NativeShellScreen::SafeKey, NativeUiIntent::SafeKeyFocusPassword) => {
                self.login.focus = LoginFocus::Password;
                self.notice = None;
                true
            }
            (NativeShellScreen::SafeKey, NativeUiIntent::SafeKeyPress { key }) => {
                if !self.safe_key.keys.contains(&key) {
                    self.set_error("invalid safe key");
                    return false;
                }
                match self.login.focus {
                    LoginFocus::Account => {
                        if self.login.account.chars().count() < 24 {
                            self.login.account.push(key.to_ascii_lowercase());
                        }
                    }
                    LoginFocus::Password => {
                        if self.login.password.chars().count() < 32 {
                            self.login.password.push(key.to_ascii_lowercase());
                        }
                    }
                    LoginFocus::LoginButton | LoginFocus::NewAccountButton => {
                        self.login.focus = LoginFocus::Account;
                    }
                }
                self.notice = None;
                true
            }
            (NativeShellScreen::SafeKey, NativeUiIntent::SafeKeyDelete) => {
                match self.login.focus {
                    LoginFocus::Account => {
                        self.login.account.pop();
                    }
                    LoginFocus::Password => {
                        self.login.password.pop();
                    }
                    LoginFocus::LoginButton | LoginFocus::NewAccountButton => {
                        self.login.focus = LoginFocus::Account;
                    }
                }
                self.notice = None;
                true
            }
            (NativeShellScreen::SafeKey, NativeUiIntent::SafeKeyRandom) => {
                self.safe_key.reshuffle();
                self.notice = None;
                true
            }
            (NativeShellScreen::SafeKey, NativeUiIntent::SafeKeyEnter) => self.begin_login(),
            (
                NativeShellScreen::CharacterSelect,
                NativeUiIntent::DeleteCharacter { character_index },
            ) => {
                if self.has_character_index(character_index) {
                    self.delete_confirm_index = Some(character_index);
                    self.delete_request_in_flight = false;
                    self.delete_command_sent = false;
                    self.screen = NativeShellScreen::DeleteConfirm {
                        index: character_index,
                    };
                    self.notice = None;
                    true
                } else {
                    self.set_error("invalid character index");
                    false
                }
            }
            (
                NativeShellScreen::DeleteConfirm { index },
                NativeUiIntent::ConfirmDeleteCharacter,
            ) => {
                if self.delete_request_in_flight {
                    return false;
                }
                if Some(index) == self.delete_confirm_index && self.has_character_index(index) {
                    self.delete_request_in_flight = true;
                    self.delete_command_sent = false;
                    self.set_info(format!("delete-character requested index={index}"));
                    true
                } else {
                    self.set_error("invalid delete confirmation");
                    false
                }
            }
            (NativeShellScreen::DeleteConfirm { .. }, NativeUiIntent::CancelDeleteCharacter) => {
                if self.delete_request_in_flight {
                    self.set_error("delete-character request is still pending");
                    return false;
                }
                self.delete_confirm_index = None;
                self.delete_command_sent = false;
                self.screen = NativeShellScreen::CharacterSelect;
                self.notice = None;
                true
            }
            (NativeShellScreen::ConnectionLost, NativeUiIntent::Retry) => {
                if self.retry_request_in_flight {
                    self.set_error("retry request is still pending");
                    return false;
                }
                self.screen = NativeShellScreen::Connecting;
                self.clear_session_payload();
                self.retry_request_in_flight = true;
                true
            }
            (NativeShellScreen::InGame, NativeUiIntent::Logout)
            | (NativeShellScreen::CharacterSelect, NativeUiIntent::Logout) => {
                if self.logout_request_in_flight {
                    self.set_error("logout request is still pending");
                    return false;
                }
                self.screen = NativeShellScreen::Login;
                self.selected_character_index = None;
                self.active_character = None;
                self.logout_request_in_flight = true;
                self.notice = None;
                self.login.clear_password();
                true
            }
            _ => {
                self.set_error("intent is not valid for this screen");
                false
            }
        }
    }

    pub fn apply_gateway_event(&mut self, event: NativeGatewayEvent) -> bool {
        match (self.screen, event) {
            (_, NativeGatewayEvent::Disconnect { reason }) => {
                self.screen = NativeShellScreen::ConnectionLost;
                self.clear_session_payload();
                self.login.clear_password();
                self.set_error(reason.unwrap_or_else(|| "connection closed".to_owned()));
                true
            }
            (NativeShellScreen::Connecting, NativeGatewayEvent::Connected) => {
                self.screen = NativeShellScreen::Login;
                self.retry_request_in_flight = false;
                self.notice = None;
                true
            }
            (NativeShellScreen::Login, NativeGatewayEvent::AccountCreated) => {
                self.register_request_in_flight = false;
                self.set_info("account created; use Login to continue");
                true
            }
            (NativeShellScreen::Login, NativeGatewayEvent::AccountCreationFailed { message }) => {
                self.register_request_in_flight = false;
                self.set_error(message);
                true
            }
            (
                NativeShellScreen::Authenticating,
                NativeGatewayEvent::LoginSuccess {
                    account,
                    characters,
                },
            ) => {
                self.login_request_in_flight = false;
                self.register_request_in_flight = false;
                self.logout_request_in_flight = false;
                self.screen = NativeShellScreen::CharacterSelect;
                self.last_account = Some(account);
                self.selected_character_index = characters.first().map(|character| character.index);
                self.characters = characters;
                self.active_character = None;
                self.login.clear_password();
                self.notice = None;
                true
            }
            (NativeShellScreen::Authenticating, NativeGatewayEvent::LoginFailure { message }) => {
                self.login_request_in_flight = false;
                self.register_request_in_flight = false;
                self.screen = NativeShellScreen::Login;
                self.set_error(message);
                true
            }
            (
                NativeShellScreen::CharacterSelect,
                NativeGatewayEvent::CharacterCreated { character },
            )
            | (
                NativeShellScreen::CharacterCreate,
                NativeGatewayEvent::CharacterCreated { character },
            ) => {
                self.create_character_request_in_flight = false;
                self.characters.retain(|item| item.index != character.index);
                self.selected_character_index = Some(character.index);
                self.characters.insert(0, character);
                self.screen = NativeShellScreen::CharacterSelect;
                true
            }
            (screen, NativeGatewayEvent::CharacterDeleted { character_index }) => match screen {
                NativeShellScreen::CharacterSelect
                | NativeShellScreen::CharacterCreate
                | NativeShellScreen::DeleteConfirm { .. } => {
                    self.characters.retain(|item| item.index != character_index);
                    if self.selected_character_index == Some(character_index) {
                        self.selected_character_index =
                            self.characters.first().map(|character| character.index);
                    }
                    self.delete_confirm_index = None;
                    self.delete_request_in_flight = false;
                    self.delete_command_sent = false;
                    self.screen = NativeShellScreen::CharacterSelect;
                    self.set_info(format!("character deleted index={character_index}"));
                    true
                }
                _ => {
                    self.set_error("delete result is ignored in this screen");
                    false
                }
            },
            (
                NativeShellScreen::StartingGame,
                NativeGatewayEvent::StartGameAck {
                    accepted: false,
                    reason,
                },
            ) => {
                self.start_game_request_in_flight = false;
                self.screen = NativeShellScreen::CharacterSelect;
                self.set_error(reason.unwrap_or_else(|| "start game rejected".to_owned()));
                true
            }
            (
                NativeShellScreen::StartingGame,
                NativeGatewayEvent::StartGameAck { accepted: true, .. },
            ) => {
                self.start_game_request_in_flight = false;
                self.set_info("start game acknowledged");
                true
            }
            (
                NativeShellScreen::StartingGame,
                NativeGatewayEvent::PlayerBootstrapped { character },
            ) => {
                self.start_game_request_in_flight = false;
                self.screen = NativeShellScreen::InGame;
                self.active_character = Some(character);
                self.set_info("entered game");
                true
            }
            (
                NativeShellScreen::ChangePassword,
                NativeGatewayEvent::ChangePasswordResult { result },
            ) => {
                self.change_password_request_in_flight = false;
                self.change_password_command_sent = false;
                match result {
                    0 => {
                        self.change_password = ChangePasswordForm::default();
                        self.screen = NativeShellScreen::Login;
                        self.set_error("password changing is disabled");
                    }
                    1 => {
                        self.change_password.focus = ChangePasswordFocus::AccountId;
                        self.set_error("account ID is invalid");
                    }
                    2 => {
                        self.change_password.focus = ChangePasswordFocus::OldPassword;
                        self.set_error("current password is invalid");
                    }
                    3 => {
                        self.change_password.focus = ChangePasswordFocus::NewPassword;
                        self.set_error("new password is invalid");
                    }
                    4 => {
                        self.change_password.focus = ChangePasswordFocus::AccountId;
                        self.set_error("account does not exist");
                    }
                    5 => {
                        self.change_password.old_password.clear();
                        self.change_password.focus = ChangePasswordFocus::OldPassword;
                        self.set_error("current password is incorrect");
                    }
                    6 => {
                        self.change_password = ChangePasswordForm::default();
                        self.screen = NativeShellScreen::Login;
                        self.set_info("password changed successfully");
                    }
                    other => {
                        self.set_error(format!("change password failed (result {other})"));
                    }
                }
                true
            }
            (
                NativeShellScreen::ChangePassword,
                NativeGatewayEvent::ChangePasswordBanned { reason, expiry },
            ) => {
                self.change_password = ChangePasswordForm::default();
                self.change_password_request_in_flight = false;
                self.change_password_command_sent = false;
                self.screen = NativeShellScreen::Login;
                let detail = expiry
                    .filter(|value| !value.is_empty())
                    .map(|value| format!("; expires {value}"))
                    .unwrap_or_default();
                self.set_error(format!("account is banned: {reason}{detail}"));
                true
            }
            (_, NativeGatewayEvent::OperationFailure { message }) => {
                match self.screen {
                    NativeShellScreen::Authenticating => {
                        self.login_request_in_flight = false;
                        self.screen = NativeShellScreen::Login;
                    }
                    NativeShellScreen::Login => {
                        self.register_request_in_flight = false;
                        self.logout_request_in_flight = false;
                    }
                    NativeShellScreen::CharacterCreate => {
                        self.create_character_request_in_flight = false;
                    }
                    NativeShellScreen::StartingGame => {
                        self.start_game_request_in_flight = false;
                        self.screen = NativeShellScreen::CharacterSelect;
                    }
                    NativeShellScreen::ChangePassword => {
                        self.change_password_request_in_flight = false;
                        self.change_password_command_sent = false;
                    }
                    NativeShellScreen::DeleteConfirm { .. } => {
                        self.delete_request_in_flight = false;
                        self.delete_command_sent = false;
                    }
                    NativeShellScreen::Connecting => {
                        self.retry_request_in_flight = false;
                    }
                    _ => {}
                }
                self.set_error(message);
                true
            }
            (_, NativeGatewayEvent::LoggedOut { characters }) => {
                self.login_request_in_flight = false;
                self.register_request_in_flight = false;
                self.create_character_request_in_flight = false;
                self.start_game_request_in_flight = false;
                self.retry_request_in_flight = false;
                self.logout_request_in_flight = false;
                self.change_password_request_in_flight = false;
                self.change_password_command_sent = false;
                self.delete_request_in_flight = false;
                self.delete_command_sent = false;
                self.screen = NativeShellScreen::Login;
                self.characters = characters;
                self.selected_character_index = None;
                self.active_character = None;
                self.notice = None;
                self.login.clear_password();
                true
            }
            _ => {
                self.set_error("gateway event is ignored for this screen");
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model_with_valid_login() -> NativeShellModel {
        let mut model = NativeShellModel::default();
        model.screen = NativeShellScreen::Login;
        model.login.account = "test-account".to_owned();
        model.login.password = "secret-pass".to_owned();
        model
    }

    fn starter_characters() -> Vec<CharacterSummary> {
        vec![
            CharacterSummary::new(1, "Warrior", 10, "Warrior", "Female"),
            CharacterSummary::new(2, "Mage", 14, "Wizard", "Male"),
        ]
    }

    #[test]
    fn missing_fields_are_rejected_for_login() {
        let mut model = NativeShellModel::default();
        model.screen = NativeShellScreen::Login;
        model.login.account = "a".to_owned();
        assert!(!model.apply_ui_intent(NativeUiIntent::Login));
        assert_eq!(model.screen, NativeShellScreen::Login);
        assert_eq!(
            model.notice.as_ref().map(|notice| notice.message.as_str()),
            Some("account and password are required")
        );

        model.login.password = "b".to_owned();
        model.login.account.clear();
        assert!(!model.apply_ui_intent(NativeUiIntent::Login));
        assert_eq!(model.screen, NativeShellScreen::Login);
    }

    #[test]
    fn login_flow_transitions_to_character_select_after_success() {
        let mut model = model_with_valid_login();
        assert!(model.apply_ui_intent(NativeUiIntent::Login));
        assert_eq!(model.screen, NativeShellScreen::Authenticating);

        assert!(model.apply_gateway_event(NativeGatewayEvent::LoginSuccess {
            account: "test-account".to_owned(),
            characters: starter_characters(),
        }));
        assert_eq!(model.screen, NativeShellScreen::CharacterSelect);
        assert_eq!(model.login.password, "");
        assert_eq!(model.characters.len(), 2);
        assert_eq!(model.last_account.as_deref(), Some("test-account"));
        assert_eq!(model.selected_character_index, Some(1));
    }

    #[test]
    fn account_registration_stays_on_login_and_surfaces_authoritative_result() {
        let mut model = model_with_valid_login();
        assert!(model.apply_ui_intent(NativeUiIntent::RegisterAccount));
        assert_eq!(model.screen, NativeShellScreen::Login);
        assert_eq!(
            model.notice.as_ref().map(|notice| notice.message.as_str()),
            Some("account creation requested")
        );

        assert!(model.apply_gateway_event(NativeGatewayEvent::AccountCreated));
        assert_eq!(model.screen, NativeShellScreen::Login);
        assert_eq!(
            model.notice.as_ref().map(|notice| notice.message.as_str()),
            Some("account created; use Login to continue")
        );

        assert!(
            model.apply_gateway_event(NativeGatewayEvent::AccountCreationFailed {
                message: "account already exists".to_owned(),
            })
        );
        assert_eq!(
            model.notice.as_ref().map(|notice| notice.message.as_str()),
            Some("account already exists")
        );
    }

    #[test]
    fn login_failure_then_disconnect_retry() {
        let mut model = model_with_valid_login();
        model.screen = NativeShellScreen::Login;
        assert!(model.apply_ui_intent(NativeUiIntent::Login));
        assert!(matches!(model.screen, NativeShellScreen::Authenticating));

        assert!(model.apply_gateway_event(NativeGatewayEvent::LoginFailure {
            message: "bad account".to_owned(),
        }));
        assert_eq!(model.screen, NativeShellScreen::Login);
        assert_eq!(
            model.notice.as_ref().map(|notice| notice.message.as_str()),
            Some("bad account")
        );

        assert!(model.apply_gateway_event(NativeGatewayEvent::Disconnect {
            reason: Some("network lost".to_owned()),
        }));
        assert_eq!(model.screen, NativeShellScreen::ConnectionLost);

        assert!(model.apply_ui_intent(NativeUiIntent::Retry));
        assert_eq!(model.screen, NativeShellScreen::Connecting);
    }

    #[test]
    fn in_game_disconnect_clears_password_and_session_payload() {
        let mut model = model_with_valid_login();
        model.screen = NativeShellScreen::InGame;
        model.login.password = "super-secret".to_owned();
        model.active_character = starter_characters().into_iter().next();
        model.characters = starter_characters();
        assert!(model.apply_gateway_event(NativeGatewayEvent::Disconnect {
            reason: Some("socket closed".to_owned()),
        }));
        assert_eq!(model.screen, NativeShellScreen::ConnectionLost);
        assert!(model.login.password.is_empty());
        assert!(model.active_character.is_none());
        assert!(model.characters.is_empty());
        let debug = format!("{model:?}");
        assert!(!debug.contains("super-secret"));
        assert!(model.apply_ui_intent(NativeUiIntent::Retry));
        assert_eq!(model.screen, NativeShellScreen::Connecting);
    }

    #[test]
    fn empty_roster_is_supported_after_login_success() {
        let mut model = model_with_valid_login();
        assert!(model.apply_ui_intent(NativeUiIntent::Login));
        assert!(model.apply_gateway_event(NativeGatewayEvent::LoginSuccess {
            account: "test-account".to_owned(),
            characters: Vec::new(),
        }));
        assert_eq!(model.screen, NativeShellScreen::CharacterSelect);
        assert_eq!(model.characters.len(), 0);
    }

    #[test]
    fn select_and_start_only_allow_valid_indices() {
        let mut model = NativeShellModel::default();
        model.screen = NativeShellScreen::CharacterSelect;
        model.characters = starter_characters();

        assert!(!model.apply_ui_intent(NativeUiIntent::SelectCharacter {
            character_index: 99
        }));
        assert_eq!(model.selected_character_index, None);

        assert!(!model.apply_ui_intent(NativeUiIntent::StartGame));
        assert!(model.apply_ui_intent(NativeUiIntent::SelectCharacter { character_index: 2 }));
        assert_eq!(model.selected_character_index, Some(2));
        model.selected_character_index = Some(1);
        assert!(model.apply_ui_intent(NativeUiIntent::StartGame));
        assert_eq!(model.screen, NativeShellScreen::StartingGame);
    }

    #[test]
    fn create_character_success_transitions_back_to_select() {
        let mut model = NativeShellModel::default();
        model.screen = NativeShellScreen::CharacterSelect;
        model.characters = starter_characters();

        assert!(model.apply_ui_intent(NativeUiIntent::OpenCharacterCreate));
        assert_eq!(model.screen, NativeShellScreen::CharacterCreate);
        assert!(model.apply_ui_intent(NativeUiIntent::CreateCharacter {
            name: "Newbie".to_owned(),
            class_name: "Warrior".to_owned(),
            gender_name: "Female".to_owned(),
        }));
        assert_eq!(model.screen, NativeShellScreen::CharacterCreate);

        assert!(
            model.apply_gateway_event(NativeGatewayEvent::CharacterCreated {
                character: CharacterSummary::new(3, "Newbie", 1, "Warrior", "Female"),
            })
        );
        assert_eq!(model.screen, NativeShellScreen::CharacterSelect);
        assert_eq!(model.characters.len(), 3);
        assert_eq!(model.characters.first().unwrap().name, "Newbie");
        assert_eq!(model.selected_character_index, Some(3));
    }

    #[test]
    fn start_game_ack_does_not_enter_game() {
        let mut model = NativeShellModel::default();
        model.screen = NativeShellScreen::StartingGame;

        assert!(model.apply_gateway_event(NativeGatewayEvent::StartGameAck {
            accepted: false,
            reason: Some("blocked".to_owned()),
        }));
        assert_eq!(model.screen, NativeShellScreen::CharacterSelect);

        model.screen = NativeShellScreen::StartingGame;

        assert!(model.apply_gateway_event(NativeGatewayEvent::StartGameAck {
            accepted: true,
            reason: None,
        }));
        assert_eq!(model.screen, NativeShellScreen::StartingGame);
        assert_eq!(
            model.notice.as_ref().map(|notice| notice.message.as_str()),
            Some("start game acknowledged")
        );
    }

    #[test]
    fn player_bootstrap_enters_game() {
        let mut model = NativeShellModel::default();
        model.screen = NativeShellScreen::StartingGame;
        let character = CharacterSummary::new(1, "Warrior", 11, "Warrior", "Female");

        assert!(
            model.apply_gateway_event(NativeGatewayEvent::PlayerBootstrapped {
                character: character.clone(),
            })
        );
        assert_eq!(model.screen, NativeShellScreen::InGame);
        assert_eq!(model.active_character.as_ref(), Some(&character));
        assert_eq!(
            model.notice.as_ref().map(|notice| notice.message.as_str()),
            Some("entered game")
        );
    }

    #[test]
    fn disconnect_and_retry_returns_to_connecting() {
        let mut model = NativeShellModel::default();
        model.screen = NativeShellScreen::InGame;
        model.active_character = Some(CharacterSummary::new(9, "Warrior", 10, "Warrior", "Female"));

        assert!(model.apply_gateway_event(NativeGatewayEvent::Disconnect {
            reason: Some("gateway closed".to_owned()),
        }));
        assert_eq!(model.screen, NativeShellScreen::ConnectionLost);
        assert_eq!(
            model.notice.as_ref().map(|notice| notice.message.as_str()),
            Some("gateway closed")
        );
        assert!(model.apply_ui_intent(NativeUiIntent::Retry));
        assert_eq!(model.screen, NativeShellScreen::Connecting);
        assert_eq!(model.characters.len(), 0);
        assert_eq!(model.selected_character_index, None);
        assert_eq!(model.active_character, None);
    }

    #[test]
    fn debug_output_never_contains_password_and_login_clears_it() {
        let mut model = model_with_valid_login();
        let before = format!("{:?}", model);
        assert!(!before.contains("secret-pass"));

        assert!(model.apply_ui_intent(NativeUiIntent::Login));
        assert!(model.apply_gateway_event(NativeGatewayEvent::LoginSuccess {
            account: "test-account".to_owned(),
            characters: Vec::new(),
        }));
        let after = format!("{:?}", model);
        assert!(!after.contains("secret-pass"));
        assert_eq!(model.login.password, "");
    }

    #[test]
    fn character_delete_updates_selection() {
        let mut model = NativeShellModel::default();
        model.screen = NativeShellScreen::CharacterSelect;
        model.characters = starter_characters();
        model.selected_character_index = Some(2);

        assert!(
            model.apply_gateway_event(NativeGatewayEvent::CharacterDeleted { character_index: 2 })
        );
        assert_eq!(model.selected_character_index, Some(1));
        assert_eq!(model.characters.len(), 1);
        assert_eq!(
            model.notice.as_ref().map(|notice| notice.message.as_str()),
            Some("character deleted index=2")
        );
    }

    #[test]
    fn character_create_can_be_cancelled_without_gateway_success() {
        let mut model = NativeShellModel::default();
        model.screen = NativeShellScreen::CharacterSelect;
        assert!(model.apply_ui_intent(NativeUiIntent::OpenCharacterCreate));
        model.character_create.name = "Unsaved".to_owned();

        assert!(model.apply_ui_intent(NativeUiIntent::CancelCharacterCreate));
        assert_eq!(model.screen, NativeShellScreen::CharacterSelect);
        assert!(model.character_create.name.is_empty());
    }

    #[test]
    fn operation_failure_returns_starting_game_to_character_select() {
        let mut model = NativeShellModel::default();
        model.screen = NativeShellScreen::StartingGame;

        assert!(
            model.apply_gateway_event(NativeGatewayEvent::OperationFailure {
                message: "start failed".to_owned(),
            })
        );
        assert_eq!(model.screen, NativeShellScreen::CharacterSelect);
        assert_eq!(
            model.notice.as_ref().map(|notice| notice.message.as_str()),
            Some("start failed")
        );
    }

    #[test]
    fn logged_out_returns_to_login_with_server_roster() {
        let mut model = NativeShellModel::default();
        model.screen = NativeShellScreen::InGame;
        model.login.password = "super-secret".to_owned();
        model.active_character = Some(CharacterSummary::new(1, "Warrior", 2, "Warrior", "Male"));
        let roster = starter_characters();

        assert!(model.apply_gateway_event(NativeGatewayEvent::LoggedOut {
            characters: roster.clone(),
        }));
        assert_eq!(model.screen, NativeShellScreen::Login);
        assert_eq!(model.characters, roster);
        assert!(model.active_character.is_none());
        assert!(model.login.password.is_empty());
        let debug = format!("{model:?}");
        assert!(!debug.contains("super-secret"));
    }

    #[test]
    fn in_game_logout_clears_password_and_does_not_auto_retry() {
        let mut model = model_with_valid_login();
        model.screen = NativeShellScreen::InGame;
        model.login.password = "super-secret".to_owned();
        model.active_character = starter_characters().into_iter().next();
        assert!(model.apply_ui_intent(NativeUiIntent::Logout));
        assert_eq!(model.screen, NativeShellScreen::Login);
        assert!(model.login.password.is_empty());
        assert!(model.active_character.is_none());
        assert_ne!(model.screen, NativeShellScreen::Connecting);
        let debug = format!("{model:?}");
        assert!(!debug.contains("super-secret"));
    }

    #[test]
    fn disconnect_does_not_auto_reconnect_without_retry() {
        let mut model = NativeShellModel::default();
        model.screen = NativeShellScreen::InGame;
        assert!(model.apply_gateway_event(NativeGatewayEvent::Disconnect {
            reason: Some("socket closed".to_owned()),
        }));
        assert_eq!(model.screen, NativeShellScreen::ConnectionLost);
        assert_ne!(model.screen, NativeShellScreen::Connecting);
        assert!(!model.apply_ui_intent(NativeUiIntent::Login));
        assert_eq!(model.screen, NativeShellScreen::ConnectionLost);
    }

    #[test]
    fn change_password_requires_account_and_matching_valid_fields() {
        let mut model = model_with_valid_login();
        assert!(model.apply_ui_intent(NativeUiIntent::OpenChangePassword));
        assert_eq!(model.change_password.account_id, "test-account");

        assert!(
            !model.apply_ui_intent(NativeUiIntent::SubmitChangePassword {
                account_id: "testaccount".to_owned(),
                old_password: "oldpw".to_owned(),
                new_password: "newpw".to_owned(),
                confirm_password: "different".to_owned(),
            })
        );
        assert!(!model.change_password_request_in_flight);
        assert_eq!(
            model.notice.as_ref().map(|notice| notice.message.as_str()),
            Some("new password confirmation does not match")
        );

        assert!(model.apply_ui_intent(NativeUiIntent::SubmitChangePassword {
            account_id: "testaccount".to_owned(),
            old_password: "oldpw".to_owned(),
            new_password: "newpw".to_owned(),
            confirm_password: "newpw".to_owned(),
        }));
        assert!(model.change_password_command_pending());

        assert!(model.apply_gateway_event(NativeGatewayEvent::ChangePasswordResult { result: 5 }));
        assert_eq!(model.screen, NativeShellScreen::ChangePassword);
        assert_eq!(
            model.change_password.focus,
            ChangePasswordFocus::OldPassword
        );
        assert!(!model.change_password_request_in_flight);
        assert_eq!(model.change_password.old_password, "");
        assert_eq!(
            model.notice.as_ref().map(|notice| notice.message.as_str()),
            Some("current password is incorrect")
        );

        assert!(model.apply_ui_intent(NativeUiIntent::SubmitChangePassword {
            account_id: "testaccount".to_owned(),
            old_password: "oldpw".to_owned(),
            new_password: "newpw".to_owned(),
            confirm_password: "newpw".to_owned(),
        }));
        assert!(model.apply_gateway_event(NativeGatewayEvent::ChangePasswordResult { result: 6 }));
        assert_eq!(model.screen, NativeShellScreen::Login);
        assert_eq!(
            model.notice.as_ref().map(|notice| notice.message.as_str()),
            Some("password changed successfully")
        );
    }

    #[test]
    fn change_password_banned_is_not_treated_as_success() {
        let mut model = model_with_valid_login();
        assert!(model.apply_ui_intent(NativeUiIntent::OpenChangePassword));
        assert!(
            model.apply_gateway_event(NativeGatewayEvent::ChangePasswordBanned {
                reason: "too many attempts".to_owned(),
                expiry: Some("2030-01-01".to_owned()),
            })
        );
        assert_eq!(model.screen, NativeShellScreen::Login);
        assert_eq!(
            model.notice.as_ref().map(|notice| notice.message.as_str()),
            Some("account is banned: too many attempts; expires 2030-01-01")
        );
        assert_ne!(
            model.notice.as_ref().map(|notice| notice.message.as_str()),
            Some("password changed successfully")
        );
    }

    #[test]
    fn safe_key_permutation_is_deterministic_and_complete() {
        let first = safe_key_permutation(7);
        let second = safe_key_permutation(7);
        assert_eq!(first, second);
        assert_eq!(first.len(), 36);

        let mut sorted = first.clone();
        sorted.sort_unstable();
        let mut expected = SAFE_KEY_ALPHABET.to_vec();
        expected.sort_unstable();
        assert_eq!(sorted, expected);
    }

    #[test]
    fn delete_confirmation_is_local_and_second_confirm_is_rejected() {
        let mut model = NativeShellModel::default();
        model.screen = NativeShellScreen::CharacterSelect;
        model.characters = vec![CharacterSummary::new(7, "Warrior", 1, "Warrior", "Male")];
        model.selected_character_index = Some(7);

        assert!(model.apply_ui_intent(NativeUiIntent::DeleteCharacter { character_index: 7 }));
        assert_eq!(model.screen, NativeShellScreen::DeleteConfirm { index: 7 });
        assert!(!model.delete_request_in_flight);
        assert!(!model.delete_command_sent);

        assert!(model.apply_ui_intent(NativeUiIntent::ConfirmDeleteCharacter));
        assert!(model.delete_command_pending().is_some());
        assert!(!model.apply_ui_intent(NativeUiIntent::ConfirmDeleteCharacter));
        model.mark_delete_command_sent();
        assert!(model.delete_command_pending().is_none());
    }
}
