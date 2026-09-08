//! OS input edits target only the currently focused shared draft.
use mir2_client_bevy::native_shell::{
    ChangePasswordFocus, CharacterCreateFocus, LoginFocus, NativeShellModel,
    NativeShellScreen as Screen,
};

pub fn shell_field(model: &NativeShellModel) -> Option<(&'static str, &str, bool)> {
    match model.screen {
        Screen::Login => match model.login.focus {
            LoginFocus::Account => Some(("account", &model.login.account, false)),
            LoginFocus::Password => Some(("password", &model.login.password, true)),
            _ => None,
        },
        Screen::CharacterCreate if model.character_create.focus == CharacterCreateFocus::Name => {
            Some(("character-name", &model.character_create.name, false))
        }
        Screen::ChangePassword => match model.change_password.focus {
            ChangePasswordFocus::AccountId => {
                Some(("change-account", &model.change_password.account_id, false))
            }
            ChangePasswordFocus::OldPassword => {
                Some(("old-password", &model.change_password.old_password, true))
            }
            ChangePasswordFocus::NewPassword => {
                Some(("new-password", &model.change_password.new_password, true))
            }
            ChangePasswordFocus::ConfirmPassword => Some((
                "confirm-password",
                &model.change_password.confirm_password,
                true,
            )),
            _ => None,
        },
        _ => None,
    }
}

pub fn edit_shell(model: &mut NativeShellModel, field: &str, text: &str) {
    if shell_field(model).map(|v| v.0) != Some(field) {
        return;
    }
    let (target, max, alphanumeric) = match field {
        "account" => (&mut model.login.account, 24, false),
        "password" => (&mut model.login.password, 32, false),
        "character-name" => (&mut model.character_create.name, 18, true),
        "change-account" => (&mut model.change_password.account_id, 15, true),
        "old-password" => (&mut model.change_password.old_password, 15, true),
        "new-password" => (&mut model.change_password.new_password, 15, true),
        "confirm-password" => (&mut model.change_password.confirm_password, 15, true),
        _ => return,
    };
    target.clear();
    for c in text.chars() {
        use mir2_client_bevy::native_shell_ui::{
            append_alphanumeric_field, append_editable_field, append_name_field,
        };
        if field == "character-name" {
            append_name_field(target, c, max);
        } else if alphanumeric {
            append_alphanumeric_field(target, c, max);
        } else {
            append_editable_field(target, c, max);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_editor_cannot_change_another_screen_or_field() {
        let mut model = NativeShellModel::default();
        model.screen = Screen::CharacterCreate;
        edit_shell(&mut model, "password", "secret");
        assert!(model.login.password.is_empty());
        edit_shell(&mut model, "character-name", "Valid123");
        assert_eq!(model.character_create.name, "Valid123");
        model.character_create.focus = CharacterCreateFocus::CancelButton;
        edit_shell(&mut model, "character-name", "stale");
        assert_eq!(model.character_create.name, "Valid123");
    }
    #[test]
    fn password_editor_is_secure_and_bounded() {
        let mut model = NativeShellModel::default();
        model.screen = Screen::ChangePassword;
        model.change_password.focus = ChangePasswordFocus::NewPassword;
        assert_eq!(shell_field(&model).unwrap().2, true);
        edit_shell(&mut model, "new-password", "12345678901234567890@");
        assert_eq!(model.change_password.new_password, "123456789012345");
    }

    #[test]
    fn character_name_uses_shared_unicode_filter() {
        let mut model = NativeShellModel::default();
        model.screen = Screen::CharacterCreate;
        edit_shell(&mut model, "character-name", "勇士 UI\n");
        assert_eq!(model.character_create.name, "勇士UI");
    }

    #[test]
    fn player_edit_is_local_bounded_and_focus_guarded() {
        let mut state = NativePlayerUiState::default();
        state.set_chat_focused(true);
        edit_player(&mut state, "chat", &"界".repeat(80));
        assert_eq!(state.chat_draft.chars().count(), 60);
        state.set_chat_focused(false);
        edit_player(&mut state, "chat", "stale");
        assert_eq!(state.chat_draft.chars().count(), 60);
    }
}

use mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState;

pub fn player_field(state: &NativePlayerUiState) -> Option<(&'static str, &str, bool)> {
    if state.amount_modal_open() {
        return None;
    }
    if state.group_open() && state.group_invite_focused {
        Some(("group-name", &state.group_invite_draft, false))
    } else if state.guild_open() && state.guild_recruit_focused {
        Some(("guild-recruit", &state.guild_recruit_draft, false))
    } else if state.guild_open() && state.guild_rank_name_focused {
        Some(("guild-rank", &state.guild_rank_name_draft, false))
    } else if state.guild_open()
        && state.guild_notice_editing
        && state.guild_notice_submission.is_none()
    {
        Some(("guild-notice", &state.guild_notice_draft, false))
    } else if state.chat_focused() {
        Some(("chat", &state.chat_draft, false))
    } else {
        None
    }
}

pub fn edit_player(state: &mut NativePlayerUiState, field: &str, text: &str) {
    if player_field(state).map(|v| v.0) != Some(field) {
        return;
    }
    let (target, limit) = match field {
        "group-name" => (&mut state.group_invite_draft, 32),
        "guild-recruit" => (&mut state.guild_recruit_draft, 32),
        "guild-rank" => (&mut state.guild_rank_name_draft, 20),
        "guild-notice" => (&mut state.guild_notice_draft, 500),
        "chat" => (&mut state.chat_draft, 60),
        _ => return,
    };
    if field == "guild-notice" {
        target.clear();
        mir2_client_bevy::crystal_ui::overlays::push_guild_notice_text(target, text);
        return;
    }
    *target = text
        .chars()
        .filter(|ch| !ch.is_control())
        .take(limit)
        .collect();
}
