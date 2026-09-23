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
    RegistrationFocus,
};

const MAX_ACCOUNT: usize = 24;
const MAX_PASSWORD: usize = 32;
const MAX_NAME: usize = 18;
const MAX_CHANGE_ACCOUNT: usize = 15;
const MAX_CHANGE_PASSWORD: usize = 15;
const MAX_REGISTRATION_TEXT: usize = 30;
const MAX_REGISTRATION_EMAIL: usize = 50;

#[derive(Default)]
pub(crate) struct ClipboardShortcutState {
    control_left: bool,
    control_right: bool,
}

impl ClipboardShortcutState {
    fn action(&mut self, key_code: KeyCode, state: ButtonState, repeat: bool) -> Option<KeyCode> {
        let pressed = state == ButtonState::Pressed;
        match key_code {
            KeyCode::ControlLeft => self.control_left = pressed,
            KeyCode::ControlRight => self.control_right = pressed,
            _ => {}
        }
        (pressed
            && !repeat
            && (self.control_left || self.control_right)
            && matches!(
                key_code,
                KeyCode::KeyV | KeyCode::KeyC | KeyCode::KeyX | KeyCode::KeyA
            ))
        .then_some(key_code)
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
        paste |= matches!(
            shortcut.action(event.key_code, event.state, event.repeat),
            Some(KeyCode::KeyV)
        );
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
        NativeShellScreen::Registration => match shell.registration.focus {
            RegistrationFocus::AccountId => append_filtered(
                &mut shell.registration.account_id,
                clipboard,
                MAX_CHANGE_ACCOUNT,
                |character| character.is_ascii_alphanumeric(),
            ),
            RegistrationFocus::Password => append_filtered(
                &mut shell.registration.password,
                clipboard,
                MAX_CHANGE_PASSWORD,
                |character| character.is_ascii_alphanumeric(),
            ),
            RegistrationFocus::ConfirmPassword => append_filtered(
                &mut shell.registration.confirm_password,
                clipboard,
                MAX_CHANGE_PASSWORD,
                |character| character.is_ascii_alphanumeric(),
            ),
            RegistrationFocus::UserName => append_filtered(
                &mut shell.registration.user_name,
                clipboard,
                20,
                |character| !character.is_control(),
            ),
            RegistrationFocus::BirthDate => append_filtered(
                &mut shell.registration.birth_date,
                clipboard,
                10,
                |character| character.is_ascii_digit() || character == '-',
            ),
            RegistrationFocus::SecretQuestion => append_filtered(
                &mut shell.registration.secret_question,
                clipboard,
                MAX_REGISTRATION_TEXT,
                |character| !character.is_control(),
            ),
            RegistrationFocus::SecretAnswer => append_filtered(
                &mut shell.registration.secret_answer,
                clipboard,
                MAX_REGISTRATION_TEXT,
                |character| !character.is_control(),
            ),
            RegistrationFocus::EmailAddress => append_filtered(
                &mut shell.registration.email_address,
                clipboard,
                MAX_REGISTRATION_EMAIL,
                |character| !character.is_control(),
            ),
            RegistrationFocus::SubmitButton | RegistrationFocus::CancelButton => false,
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
        NativeShellScreen::Registration => matches!(
            shell.registration.focus,
            RegistrationFocus::AccountId
                | RegistrationFocus::Password
                | RegistrationFocus::ConfirmPassword
                | RegistrationFocus::UserName
                | RegistrationFocus::BirthDate
                | RegistrationFocus::SecretQuestion
                | RegistrationFocus::SecretAnswer
                | RegistrationFocus::EmailAddress
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

/// OS reads may resolve after the compose surface was closed or retargeted.
#[derive(Default)]
pub(crate) struct MailClipboardPending(Option<(MailTextTarget, bevy::clipboard::ClipboardRead)>);

fn mail_paste_is_current(pending: &MailTextTarget, current: Option<&MailTextTarget>) -> bool {
    current == Some(pending)
}

use mir2_client_bevy::crystal_ui::overlays::text_input::{
    editor_mut, editor_owner, friend_clipboard_target, sync_draft, EditorOwner,
};
use mir2_client_bevy::crystal_ui::overlays::{
    mail_editor::MailLetterEditor,
    mail_text_adapter::{active_mail_text_target, MailTextTarget},
    MailComposeUi,
};
use mir2_client_bevy::pending_operations::{PendingOperations, SessionResetRevision};

/// Clipboard operations require a focused field. Pending reads are bound to a
/// globally unique editor instance and are cancelled on focus/modal/session changes.
pub fn paste_system(
    mut keyboard_inputs: MessageReader<KeyboardInput>,
    mut shortcut: Local<ClipboardShortcutState>,
    mut pending: Local<FriendClipboardPending>,
    mut mail_pending: Local<MailClipboardPending>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut clipboard: Option<ResMut<bevy::clipboard::Clipboard>>,
    mut shell: Option<ResMut<NativeShellModel>>,
    mut ui: Option<ResMut<mir2_client_bevy::crystal_ui::NativePlayerUiState>>,
    compose: Option<Res<MailComposeUi>>,
    mut mail_editor: Option<ResMut<MailLetterEditor>>,
    pending_operations: Option<Res<PendingOperations>>,
    reset: Option<Res<SessionResetRevision>>,
) {
    if !windows.iter().any(|window| window.focused) {
        *shortcut = ClipboardShortcutState::default();
        pending.0 = None;
        mail_pending.0 = None;
        keyboard_inputs.read().for_each(drop);
        return;
    }
    let mut actions = Vec::new();
    for event in keyboard_inputs.read() {
        if let Some(action) = shortcut.action(event.key_code, event.state, event.repeat) {
            actions.push(action);
        }
    }
    let ingame = shell
        .as_deref()
        .is_some_and(|s| s.screen == NativeShellScreen::InGame);
    if ingame {
        let Some(ui) = ui.as_deref_mut() else {
            pending.0 = None;
            mail_pending.0 = None;
            return;
        };
        ui.friends.sync_editor();
        ui.creature.sync_input_editor();
        ui.social_bonds.sync_editor();
        ui.game_shop_dialog.sync_search_editor();
        if ui.ime_frame_consumed {
            pending.0 = None;
            mail_pending.0 = None;
            return;
        }
        let session_revision = reset.as_deref().map_or(0, |revision| revision.0);
        let mail_target = match (
            compose.as_deref(),
            mail_editor.as_deref(),
            pending_operations.as_deref(),
        ) {
            (Some(compose), Some(editor), Some(pending_operations)) => active_mail_text_target(
                shell.as_deref().expect("in-game shell"),
                ui,
                compose,
                editor,
                pending_operations,
                session_revision,
            ),
            _ => None,
        };
        if let Some(mail_target) = mail_target {
            pending.0 = None;
            let Some(editor) = mail_editor.as_deref_mut() else {
                mail_pending.0 = None;
                return;
            };
            let Some(draft) = ui.core.mail_compose.as_mut() else {
                mail_pending.0 = None;
                return;
            };
            if mail_pending
                .0
                .as_ref()
                .is_some_and(|(target, _)| target != &mail_target)
            {
                mail_pending.0 = None;
            }
            for action in actions {
                // Ctrl shortcut literals also reach the client editor; its
                // Ctrl+C/X/V branch deliberately treats those as host-owned.
                match action {
                    KeyCode::KeyA => editor.select_all(),
                    KeyCode::KeyV => {
                        if let Some(clipboard) = clipboard.as_deref_mut() {
                            mail_pending.0 = Some((mail_target.clone(), clipboard.fetch_text()));
                        }
                    }
                    KeyCode::KeyC | KeyCode::KeyX => {
                        if let Some(clipboard) = clipboard.as_deref_mut() {
                            let selected = editor.selected_text().to_owned();
                            if !selected.is_empty()
                                && clipboard.set_text(selected).is_ok()
                                && action == KeyCode::KeyX
                            {
                                editor.cut_selection(draft);
                            }
                        }
                    }
                    _ => {}
                }
            }
            if let Some((target, read)) = mail_pending.0.as_mut() {
                if !mail_paste_is_current(target, Some(&mail_target)) {
                    mail_pending.0 = None;
                } else if let Some(result) = read.poll_result() {
                    mail_pending.0 = None;
                    if let Ok(text) = result {
                        editor.paste(draft, &text);
                    }
                }
            }
            return;
        }
        mail_pending.0 = None;
        let Some(owner) = editor_owner(ui) else {
            pending.0 = None;
            return;
        };
        let editor = editor_mut(ui, owner);
        let revision = editor.editor_revision;
        if pending.0.as_ref().is_some_and(|(r, _)| *r != revision) {
            pending.0 = None;
        }
        for action in actions {
            // Shortcut literals are emitted by Windows as normal text input;
            // consume them even when the host clipboard is unavailable.
            editor.input_consumed = true;
            match action {
                KeyCode::KeyA => {
                    if let Some(editor) = editor.editor.as_mut() {
                        editor.select_all();
                    }
                }
                KeyCode::KeyV => {
                    if let Some(clipboard) = clipboard.as_deref_mut() {
                        pending.0 = Some((revision, clipboard.fetch_text()));
                    }
                }
                KeyCode::KeyC | KeyCode::KeyX => {
                    if let Some(clipboard) = clipboard.as_deref_mut() {
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
                }
                _ => {}
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
        sync_draft(ui, owner);
        return;
    }
    pending.0 = None;
    mail_pending.0 = None;
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
    fn game_shop_clipboard_owner_is_focus_and_revision_bound() {
        let mut ui = mir2_client_bevy::crystal_ui::NativePlayerUiState::default();
        ui.core.panel = mir2_ui_core::state::UiPanel::GameShop;
        ui.game_shop_page = 6;
        ui.game_shop_dialog.search_focused = true;
        ui.game_shop_dialog.sync_search_editor();
        let revision = ui.game_shop_dialog.search_input.editor_revision;
        assert_eq!(editor_owner(&ui), Some(EditorOwner::GameShop));

        let editor = editor_mut(&mut ui, EditorOwner::GameShop);
        editor.paste("RedTiger");
        sync_draft(&mut ui, EditorOwner::GameShop);
        assert_eq!(ui.game_shop_dialog.search, "RedTiger");
        assert_eq!(ui.game_shop_page, 0);

        ui.game_shop_dialog.blur_search();
        assert_eq!(editor_owner(&ui), None);
        ui.game_shop_dialog.search_focused = true;
        ui.game_shop_dialog.sync_search_editor();
        assert_ne!(ui.game_shop_dialog.search_input.editor_revision, revision);
        ui.game_shop_dialog.confirmation = Some(
            mir2_client_bevy::crystal_ui::overlays::game_shop_dialog::PurchasePrompt {
                index: 1,
                name: "RedTiger".into(),
                quantity: 1,
                count: 1,
                payment: mir2_client_bevy::game_shop::GameShopPaymentType::Gold,
                total: 1,
            },
        );
        assert_eq!(editor_owner(&ui), None);
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
        assert_eq!(
            shortcut.action(KeyCode::ControlLeft, ButtonState::Pressed, false),
            None
        );
        assert_eq!(
            shortcut.action(KeyCode::KeyV, ButtonState::Pressed, false),
            Some(KeyCode::KeyV)
        );
        assert_eq!(
            shortcut.action(KeyCode::KeyV, ButtonState::Released, false),
            None
        );
        assert_eq!(
            shortcut.action(KeyCode::ControlLeft, ButtonState::Released, false),
            None
        );
        assert!(!shortcut.control_left);
        assert!(!shortcut.control_right);
    }

    #[test]
    fn ctrl_a_is_an_ordered_editor_action_and_not_a_text_literal() {
        let mut shortcut = ClipboardShortcutState::default();
        assert_eq!(
            shortcut.action(KeyCode::ControlRight, ButtonState::Pressed, false),
            None
        );
        assert_eq!(
            shortcut.action(KeyCode::KeyA, ButtonState::Pressed, false),
            Some(KeyCode::KeyA)
        );
        assert_eq!(
            shortcut.action(KeyCode::KeyA, ButtonState::Released, false),
            None
        );
        assert_eq!(
            shortcut.action(KeyCode::ControlRight, ButtonState::Released, false),
            None
        );
    }

    #[test]
    fn focus_loss_clears_any_latched_control_modifier() {
        let mut shortcut = ClipboardShortcutState::default();
        assert_eq!(
            shortcut.action(KeyCode::ControlLeft, ButtonState::Pressed, false),
            None
        );
        shortcut = ClipboardShortcutState::default();
        assert_eq!(
            shortcut.action(KeyCode::KeyV, ButtonState::Pressed, false),
            None
        );
    }

    #[test]
    fn delayed_mail_paste_requires_exact_draft_kind_recipient_revision_and_session() {
        let target = MailTextTarget {
            kind: mir2_client_bevy::crystal_ui::overlays::MailComposeKind::Letter,
            recipient: "Receiver".into(),
            draft_epoch: 7,
            editor_revision: 11,
            session_revision: 3,
        };
        assert!(mail_paste_is_current(&target, Some(&target)));
        for stale in [
            MailTextTarget { recipient: "Other".into(), ..target.clone() },
            MailTextTarget {
                kind: mir2_client_bevy::crystal_ui::overlays::MailComposeKind::Parcel,
                ..target.clone()
            },
            MailTextTarget { draft_epoch: 8, ..target.clone() },
            MailTextTarget { editor_revision: 12, ..target.clone() },
            MailTextTarget { session_revision: 4, ..target.clone() },
        ] {
            assert!(!mail_paste_is_current(&target, Some(&stale)));
        }
        assert!(!mail_paste_is_current(&target, None));
    }

}
