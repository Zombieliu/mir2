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

#[derive(Default)]
pub(crate) struct FriendClipboardPending(Option<(u64, bevy::clipboard::ClipboardRead)>);

use mir2_client_bevy::crystal_ui::overlays::text_input::{
    editor_mut, editor_owner, friend_clipboard_target, EditorOwner,
};

/// Clipboard operations require a focused field. Pending reads are bound to a
/// globally unique editor instance and are cancelled on focus/modal/session changes.
pub fn paste_system(
    mut keyboard_inputs: MessageReader<KeyboardInput>,
    mut shortcut: Local<ClipboardShortcutState>,
    mut pending: Local<FriendClipboardPending>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut clipboard: Option<ResMut<bevy::clipboard::Clipboard>>,
    mut shell: Option<ResMut<NativeShellModel>>,
    mut ui: Option<ResMut<mir2_client_bevy::crystal_ui::NativePlayerUiState>>,
) {
    if !windows.iter().any(|window| window.focused) {
        *shortcut = ClipboardShortcutState::default();
        pending.0 = None;
        keyboard_inputs.read().for_each(drop);
        return;
    }
    let mut actions = Vec::new();
    for event in keyboard_inputs.read() {
        let paste = shortcut.observe(event.key_code, event.state, event.repeat);
        if paste {
            actions.push(KeyCode::KeyV);
        } else if event.state == ButtonState::Pressed
            && !event.repeat
            && (shortcut.control_left || shortcut.control_right)
            && matches!(event.key_code, KeyCode::KeyC | KeyCode::KeyX)
        {
            actions.push(event.key_code);
        }
    }
    let ingame = shell
        .as_deref()
        .is_some_and(|s| s.screen == NativeShellScreen::InGame);
    if ingame {
        let Some(ui) = ui.as_deref_mut() else {
            pending.0 = None;
            return;
        };
        ui.friends.sync_editor();
        ui.creature.sync_input_editor();
        ui.social_bonds.sync_editor();
        if ui.ime_frame_consumed {
            pending.0 = None;
            return;
        }
        let Some(owner) = editor_owner(ui) else {
            pending.0 = None;
            return;
        };
        let editor = editor_mut(ui, owner);
        let revision = editor.editor_revision;
        if pending.0.as_ref().is_some_and(|(r, _)| *r != revision) {
            pending.0 = None;
        }
        if let Some(clipboard) = clipboard.as_deref_mut() {
            for action in actions {
                editor.input_consumed = true;
                match action {
                    KeyCode::KeyV => pending.0 = Some((revision, clipboard.fetch_text())),
                    KeyCode::KeyC | KeyCode::KeyX => {
                        let selected = editor
                            .editor
                            .as_ref()
                            .map(|e| e.selected_text().to_owned())
                            .unwrap_or_default();
                        if !selected.is_empty()
                            && clipboard.set_text(selected).is_ok()
                            && action == KeyCode::KeyX
                        {
                            let result = editor.editor.as_mut().unwrap().delete(false);
                            editor.commit_editor(result);
                        }
                    }
                    _ => {}
                }
            }
        }
        if let Some((_, read)) = pending.0.as_mut() {
            if let Some(result) = read.poll_result() {
                pending.0 = None;
                if let Ok(text) = result {
                    editor.paste(&text);
                    editor.input_consumed = true;
                }
            }
        }
        match owner {
            EditorOwner::GuildNotice => ui.guild_notice_draft = ui.guild_panel.notice_draft(),
            EditorOwner::GuildRank => {
                ui.guild_rank_name_draft = ui
                    .guild_panel
                    .rank_editor
                    .editor
                    .as_ref()
                    .map(|e| e.text().to_owned())
                    .unwrap_or_default()
            }
            EditorOwner::GuildRecruit => {
                ui.guild_recruit_draft = ui
                    .guild_panel
                    .recruit_editor
                    .editor
                    .as_ref()
                    .map(|e| e.text().to_owned())
                    .unwrap_or_default()
            }
            EditorOwner::Creature => ui.creature.sync_input_draft(),
            EditorOwner::Bond => ui.social_bonds.sync_draft(),
            EditorOwner::Friend | EditorOwner::Group => {}
        }
        return;
    }
    pending.0 = None;
    if !actions.contains(&KeyCode::KeyV) {
        return;
    }
    if !shell.as_deref().is_some_and(shell_has_clipboard_target) {
        return;
    }
    let Some(clipboard) = clipboard.as_deref_mut() else {
        return;
    };
    let mut read = clipboard.fetch_text();
    let Some(Ok(text)) = read.poll_result() else {
        return;
    };
    if let Some(shell) = shell.as_deref_mut() {
        apply_shell_clipboard(shell, &text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_client_bevy::native_shell::NativeShellScreen;

    #[test]
    fn group_invitation_blocks_covered_editor_and_session_reset_discards_it() {
        let mut ui = mir2_client_bevy::crystal_ui::NativePlayerUiState::default();
        ui.group_dialog
            .show_input(false, &Default::default(), "Owner");
        ui.group_dialog.editor.editor_focused = true;
        assert!(matches!(editor_owner(&ui), Some(EditorOwner::Group)));
        ui.group_dialog.invitation = Some(("Peer".into(), 1));
        assert!(editor_owner(&ui).is_none());
        ui.reset_session();
        assert!(ui.group_dialog.invitation.is_none());
        assert!(editor_owner(&ui).is_none());
    }

    #[test]
    fn unified_clipboard_owner_respects_invitation_and_creature_notice_priority() {
        use mir2_client_bevy::crystal_ui::overlays::{
            creature_dialog::{CreatureInput, InputPurpose},
            social_bond_dialog::{BondAction, BondPage},
        };
        let mut ui = mir2_client_bevy::crystal_ui::NativePlayerUiState::default();
        ui.creature.input = Some(CreatureInput {
            purpose: InputPurpose::Rename,
            pet_type: 0,
            name: "Pig".into(),
            text: "Pig".into(),
        });
        ui.creature.sync_input_editor();
        assert!(matches!(editor_owner(&ui), Some(EditorOwner::Creature)));
        ui.creature.notice = Some("Validation".into());
        assert!(editor_owner(&ui).is_none());
        ui.creature.notice = None;
        ui.social_bonds.show(BondPage::Mentor);
        ui.social_bonds
            .action(BondPage::Mentor, BondAction::AddMentor);
        ui.social_bonds.sync_editor();
        assert!(matches!(editor_owner(&ui), Some(EditorOwner::Bond)));
        let old = ui.social_bonds.input.editor_revision;
        ui.reset_session();
        ui.social_bonds.show(BondPage::Mentor);
        ui.social_bonds
            .action(BondPage::Mentor, BondAction::AddMentor);
        ui.social_bonds.sync_editor();
        assert_ne!(old, ui.social_bonds.input.editor_revision);
    }

    #[test]
    fn friend_clipboard_requires_focused_topmost_editor() {
        use mir2_client_bevy::crystal_ui::overlays::friend_dialog::FriendAction;
        let mut ui = mir2_client_bevy::crystal_ui::NativePlayerUiState::default();
        ui.friends.open = true;
        ui.friends.action(FriendAction::Add);
        ui.friends.sync_editor();
        assert!(friend_clipboard_target(&ui));
        ui.keyboard.open = true;
        assert!(!friend_clipboard_target(&ui));
        ui.keyboard.open = false;
        ui.friends.editor_focused = false;
        assert!(!friend_clipboard_target(&ui));
        ui.friends.editor_focused = true;
        ui.friends.cancel_modal();
        assert!(!friend_clipboard_target(&ui));
    }

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
