//! Windows clipboard integration for the native text fields.
//!
//! The Bevy client deliberately does not own operating-system APIs.  This
//! host adapter reads CF_UNICODETEXT only for an explicit Ctrl+V in a focused
//! editable field and immediately applies the filtered value.  Clipboard
//! contents are never logged, persisted, or rendered outside the target field.

use bevy::input::{keyboard::KeyboardInput, ButtonState};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use mir2_client_bevy::native_shell::{
    ChangePasswordFocus, CharacterCreateFocus, LoginFocus, NativeShellModel, NativeShellScreen,
};

const MAX_ACCOUNT: usize = 24;
const MAX_PASSWORD: usize = 32;
const MAX_NAME: usize = 18;
const MAX_CHANGE_ACCOUNT: usize = 15;
const MAX_CHANGE_PASSWORD: usize = 15;

#[derive(Default)]
pub(crate) struct ClipboardShortcutState {
    control_left: bool,
    control_right: bool,
}

impl ClipboardShortcutState {
    fn observe(&mut self, key_code: KeyCode, state: ButtonState, repeat: bool) -> bool {
        let pressed = state == ButtonState::Pressed;
        match key_code {
            KeyCode::ControlLeft => self.control_left = pressed,
            KeyCode::ControlRight => self.control_right = pressed,
            KeyCode::KeyV => {
                return pressed && !repeat && (self.control_left || self.control_right);
            }
            _ => {}
        }
        false
    }
}

fn paste_shortcut_pressed<'a>(
    events: impl IntoIterator<Item = &'a KeyboardInput>,
    shortcut: &mut ClipboardShortcutState,
) -> bool {
    // Process the ordered raw event stream rather than sampling ButtonInput at
    // the end of the frame. A fast Windows SendInput burst can press and
    // release Ctrl+V between two Bevy updates; ButtonInput then reports both
    // keys released even though a valid paste chord occurred.
    let mut paste = false;
    for event in events {
        paste |= shortcut.observe(event.key_code, event.state, event.repeat);
    }
    paste
}

/// Applies a clipboard value using the same field policies as ordinary
/// keyboard input.  This helper is platform-neutral so its safety properties
/// can be tested without accessing the host clipboard.
pub fn apply_shell_clipboard(shell: &mut NativeShellModel, clipboard: &str) -> bool {
    match shell.screen {
        NativeShellScreen::Login => match shell.login.focus {
            LoginFocus::Account => append_filtered(
                &mut shell.login.account,
                clipboard,
                MAX_ACCOUNT,
                |character| !character.is_control(),
            ),
            LoginFocus::Password => append_filtered(
                &mut shell.login.password,
                clipboard,
                MAX_PASSWORD,
                |character| !character.is_control(),
            ),
            LoginFocus::LoginButton | LoginFocus::NewAccountButton => false,
        },
        NativeShellScreen::CharacterCreate => {
            if shell.character_create.focus != CharacterCreateFocus::Name {
                return false;
            }
            append_filtered(
                &mut shell.character_create.name,
                clipboard,
                MAX_NAME,
                |character| !character.is_control() && !character.is_whitespace(),
            )
        }
        NativeShellScreen::ChangePassword => match shell.change_password.focus {
            ChangePasswordFocus::AccountId => append_filtered(
                &mut shell.change_password.account_id,
                clipboard,
                MAX_CHANGE_ACCOUNT,
                |character| character.is_ascii_alphanumeric(),
            ),
            ChangePasswordFocus::OldPassword => append_filtered(
                &mut shell.change_password.old_password,
                clipboard,
                MAX_CHANGE_PASSWORD,
                |character| character.is_ascii_alphanumeric(),
            ),
            ChangePasswordFocus::NewPassword => append_filtered(
                &mut shell.change_password.new_password,
                clipboard,
                MAX_CHANGE_PASSWORD,
                |character| character.is_ascii_alphanumeric(),
            ),
            ChangePasswordFocus::ConfirmPassword => append_filtered(
                &mut shell.change_password.confirm_password,
                clipboard,
                MAX_CHANGE_PASSWORD,
                |character| character.is_ascii_alphanumeric(),
            ),
            ChangePasswordFocus::SubmitButton | ChangePasswordFocus::CancelButton => false,
        },
        // SafeKey is a button grid, not a text field.  In particular, never
        // paste a secret into the account/password preview used by that panel.
        _ => false,
    }
}

fn shell_has_clipboard_target(shell: &NativeShellModel) -> bool {
    match shell.screen {
        NativeShellScreen::Login => {
            matches!(
                shell.login.focus,
                LoginFocus::Account | LoginFocus::Password
            )
        }
        NativeShellScreen::CharacterCreate => {
            shell.character_create.focus == CharacterCreateFocus::Name
        }
        NativeShellScreen::ChangePassword => matches!(
            shell.change_password.focus,
            ChangePasswordFocus::AccountId
                | ChangePasswordFocus::OldPassword
                | ChangePasswordFocus::NewPassword
                | ChangePasswordFocus::ConfirmPassword
        ),
        _ => false,
    }
}

fn append_filtered(
    destination: &mut String,
    clipboard: &str,
    max_chars: usize,
    mut allowed: impl FnMut(char) -> bool,
) -> bool {
    let before = destination.len();
    for character in clipboard.chars() {
        if destination.chars().count() >= max_chars {
            break;
        }
        if allowed(character) {
            destination.push(character);
        }
    }
    destination.len() != before
}

/// Reads and applies one Windows Ctrl+V operation. Bevy's system clipboard
/// adapter uses CF_UNICODETEXT on Windows and reports unavailable/non-text
/// content as an error, which this system intentionally ignores.
pub fn paste_system(
    mut keyboard_inputs: MessageReader<KeyboardInput>,
    mut shortcut: Local<ClipboardShortcutState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut clipboard: Option<ResMut<bevy::clipboard::Clipboard>>,
    mut shell: Option<ResMut<NativeShellModel>>,
) {
    if !windows.iter().any(|window| window.focused) {
        // Windows can omit a modifier release when focus changes. Clear the
        // host-side latch and drain this frame's events so a later plain `V`
        // cannot be mistaken for a paste shortcut after focus returns.
        *shortcut = ClipboardShortcutState::default();
        keyboard_inputs.read().for_each(drop);
        return;
    }
    if !paste_shortcut_pressed(keyboard_inputs.read(), &mut shortcut) {
        return;
    }
    if !shell.as_deref().is_some_and(|model| {
        model.screen != NativeShellScreen::InGame && shell_has_clipboard_target(model)
    }) {
        return;
    }
    let Some(mut clipboard) = clipboard.take() else {
        return;
    };
    let mut read = clipboard.fetch_text();
    let Some(Ok(clipboard)) = read.poll_result() else {
        return;
    };
    let Some(shell) = shell.as_deref_mut() else {
        return;
    };
    debug_assert!(shell.screen != NativeShellScreen::InGame);
    debug_assert!(shell_has_clipboard_target(shell));
    let _ = apply_shell_clipboard(shell, &clipboard);
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_client_bevy::native_shell::NativeShellScreen;

    #[test]
    fn login_account_paste_uses_existing_printable_and_length_rules() {
        let mut shell = NativeShellModel {
            screen: NativeShellScreen::Login,
            ..Default::default()
        };
        assert!(apply_shell_clipboard(&mut shell, "ab-中\ncd!"));
        assert_eq!(shell.login.account, "ab-中cd!");
        shell.login.account = "a".repeat(MAX_ACCOUNT);
        assert!(!apply_shell_clipboard(&mut shell, "more"));
        assert_eq!(shell.login.account.chars().count(), MAX_ACCOUNT);
    }

    #[test]
    fn login_password_paste_filters_controls_without_logging_or_echoing() {
        let mut shell = NativeShellModel {
            screen: NativeShellScreen::Login,
            ..Default::default()
        };
        shell.login.focus = LoginFocus::Password;
        assert!(apply_shell_clipboard(&mut shell, "p\u{0000}a\r\ns!"));
        assert_eq!(shell.login.password, "pas!");
    }

    #[test]
    fn paste_is_ignored_for_unfocused_shell_controls() {
        let mut shell = NativeShellModel {
            screen: NativeShellScreen::Login,
            ..Default::default()
        };
        shell.login.focus = LoginFocus::LoginButton;
        assert!(!apply_shell_clipboard(&mut shell, "secret"));
        assert!(shell.login.account.is_empty());
        assert!(shell.login.password.is_empty());
    }

    #[test]
    fn character_name_paste_keeps_its_field_specific_limits() {
        let mut shell = NativeShellModel {
            screen: NativeShellScreen::CharacterCreate,
            ..Default::default()
        };
        assert!(apply_shell_clipboard(&mut shell, "Hero Name\n1"));
        assert_eq!(shell.character_create.name, "HeroName1");
    }

    #[test]
    fn clipboard_value_is_not_applied_to_unfocused_or_safe_key_surfaces() {
        let mut shell = NativeShellModel {
            screen: NativeShellScreen::SafeKey,
            ..Default::default()
        };
        assert!(!apply_shell_clipboard(&mut shell, "secret"));
    }

    #[test]
    fn ctrl_v_is_detected_when_the_complete_chord_arrives_in_one_frame() {
        let mut shortcut = ClipboardShortcutState::default();
        assert!(!shortcut.observe(KeyCode::ControlLeft, ButtonState::Pressed, false));
        assert!(shortcut.observe(KeyCode::KeyV, ButtonState::Pressed, false));
        assert!(!shortcut.observe(KeyCode::KeyV, ButtonState::Released, false));
        assert!(!shortcut.observe(KeyCode::ControlLeft, ButtonState::Released, false));
        assert!(!shortcut.control_left);
        assert!(!shortcut.control_right);
    }

    #[test]
    fn focus_loss_clears_any_latched_control_modifier() {
        let mut shortcut = ClipboardShortcutState::default();
        assert!(!shortcut.observe(KeyCode::ControlLeft, ButtonState::Pressed, false));
        shortcut = ClipboardShortcutState::default();
        assert!(!shortcut.observe(KeyCode::KeyV, ButtonState::Pressed, false));
    }
}
