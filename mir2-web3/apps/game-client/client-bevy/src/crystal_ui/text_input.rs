//! Shared topmost text ownership and native IME composition. Preedit is presentation,
//! never an authoritative draft or a packet. Exact-key dialogs deliberately disable IME.
use super::*;
use bevy::window::Ime;

pub fn friend_clipboard_target(ui: &NativePlayerUiState) -> bool {
    ui.friends.open
        && ui.friends.modal.is_some()
        && ui.friends.editor_focused
        && !ui.keyboard.open
        && !ui.non_friend_amount_modal_open()
        && ui.trade_dialog.message.is_none()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorOwner {
    Friend,
    Creature,
    Bond,
    Group,
    GuildNotice,
    GuildRank,
    GuildRecruit,
}
pub fn editor_owner(ui: &NativePlayerUiState) -> Option<EditorOwner> {
    // Match the modal keyboard dispatcher: network prompts can cover an open
    // key configuration page. That covered page must not disable their IME.
    if ui.leave_game.blocks() {
        return None;
    }
    if ui.social_bonds.prompt.is_some() {
        return ui
            .social_bonds
            .input
            .editor_focused
            .then_some(EditorOwner::Bond);
    }
    if ui.group_dialog.invitation.is_some() {
        return None;
    }
    if ui.group_dialog.modal() {
        return ui
            .group_dialog
            .editor
            .editor_focused
            .then_some(EditorOwner::Group);
    }
    if ui.guild_panel.blocks() {
        return None;
    }
    if ui.creature.notice.is_some() || ui.equipment_dialogs.notice.is_some() {
        return None;
    }
    if ui.creature.input.is_some() {
        return ui
            .creature
            .text_input
            .editor_focused
            .then_some(EditorOwner::Creature);
    }
    if ui.keyboard.open
        || ui.hero.modal()
        || ui.skill_assign.open
        || ui.trade_dialog.message.is_some()
        || ui.non_friend_amount_modal_open()
    {
        return None;
    }
    if ui.friends.modal.is_none()
        && ui.guild_open()
        && ui.guild_notice_editing
        && ui.guild_notice_submission.is_none()
        && ui.guild_panel.notice_editor.editor_focused
    {
        return Some(EditorOwner::GuildNotice);
    }
    if ui.friends.modal.is_none()
        && ui.guild_open()
        && ui.guild_rank_name_focused
        && ui.guild_panel.rank_editor.editor_focused
    {
        return Some(EditorOwner::GuildRank);
    }
    if ui.friends.modal.is_none()
        && ui.guild_open()
        && ui.guild_recruit_focused
        && ui.guild_panel.recruit_editor.editor_focused
    {
        return Some(EditorOwner::GuildRecruit);
    }
    friend_clipboard_target(ui).then_some(EditorOwner::Friend)
}
pub fn editor_mut(
    ui: &mut NativePlayerUiState,
    owner: EditorOwner,
) -> &mut friend_dialog::FriendDialogUi {
    match owner {
        EditorOwner::Friend => &mut ui.friends,
        EditorOwner::Creature => &mut ui.creature.text_input,
        EditorOwner::Bond => &mut ui.social_bonds.input,
        EditorOwner::Group => &mut ui.group_dialog.editor,
        EditorOwner::GuildNotice => &mut ui.guild_panel.notice_editor,
        EditorOwner::GuildRank => &mut ui.guild_panel.rank_editor,
        EditorOwner::GuildRecruit => &mut ui.guild_panel.recruit_editor,
    }
}

pub fn sync_draft(ui: &mut NativePlayerUiState, owner: EditorOwner) {
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
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Composition {
    pub value: String,
    pub cursor: Option<(usize, usize)>,
}
#[derive(Default, Resource)]
pub struct ImeState {
    lease: Option<(EditorOwner, u64)>,
    enabled: bool,
}

pub fn process_ime(
    mut ui: ResMut<NativePlayerUiState>,
    shell: Res<NativeShellModel>,
    mut state: ResMut<ImeState>,
    mut events: MessageReader<Ime>,
    mut windows: Query<(Entity, &mut Window), With<PrimaryWindow>>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
) {
    ui.ime_frame_consumed = false;
    let events: Vec<_> = events.read().cloned().collect();
    ui.friends.sync_editor();
    ui.creature.sync_input_editor();
    ui.social_bonds.sync_editor();
    let Ok((window_id, mut window)) = windows.single_mut() else {
        return;
    };
    let owner = (window.focused && shell.screen == NativeShellScreen::InGame)
        .then(|| editor_owner(&ui))
        .flatten();
    let lease = owner.map(|owner| (owner, editor_mut(&mut ui, owner).editor_revision));
    if state.lease != lease {
        if let Some((old, _)) = state.lease {
            let editor = editor_mut(&mut ui, old);
            editor.composition = None;
            editor.modifiers = [false; 4];
        }
        state.lease = lease;
        state.enabled = false;
        window.ime_enabled = false;
        // Disable for one update before a new owner is enabled. Drop late prior-owner commits.
        return;
    }
    let Some(owner) = owner else {
        window.ime_enabled = false;
        state.enabled = false;
        return;
    };
    window.ime_enabled = true;
    // Clicking while composing cancels that uncommitted presentation before focus can move.
    let clicked = mouse
        .as_deref()
        .is_some_and(|m| m.just_pressed(MouseButton::Left) || m.just_pressed(MouseButton::Right));
    let had_composition = editor_mut(&mut ui, owner).composition.is_some();
    if had_composition
        && (clicked
            || keys
                .as_deref()
                .is_some_and(|k| k.just_pressed(KeyCode::Escape)))
    {
        editor_mut(&mut ui, owner).composition = None;
        window.ime_enabled = false;
        state.enabled = false;
        ui.ime_frame_consumed = true;
        return;
    }
    ui.ime_frame_consumed = had_composition;
    for event in events {
        match event {
            Ime::Enabled { window } if window == window_id => state.enabled = true,
            Ime::Disabled { window } if window == window_id => {
                state.enabled = false;
                editor_mut(&mut ui, owner).composition = None;
            }
            Ime::Preedit {
                window,
                value,
                cursor,
            } if window == window_id && state.enabled => {
                let editor = editor_mut(&mut ui, owner);
                editor.modifiers = [false; 4];
                editor.composition = (!value.is_empty()).then_some(Composition { value, cursor });
                ui.ime_frame_consumed = true;
            }
            Ime::Commit { window, value } if window == window_id && state.enabled => {
                let editor = editor_mut(&mut ui, owner);
                editor.composition = None;
                editor.paste(&value);
                sync_draft(&mut ui, owner);
                ui.ime_frame_consumed = true;
            }
            _ => {}
        }
    }
}

/// Candidate location follows actual laid-out glyph geometry, including DPI and scrolling.
pub fn position_ime(
    ui: Res<NativePlayerUiState>,
    state: Res<ImeState>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    texts: Query<(
        &friend_dialog::view::FriendEditText,
        &ComputedNode,
        &bevy::ui::UiGlobalTransform,
    )>,
) {
    let Some((owner, revision)) = state.lease else {
        return;
    };
    if editor_owner(&ui) != Some(owner) {
        return;
    }
    let model = match owner {
        EditorOwner::Friend => &ui.friends,
        EditorOwner::Creature => &ui.creature.text_input,
        EditorOwner::Bond => &ui.social_bonds.input,
        EditorOwner::Group => &ui.group_dialog.editor,
        EditorOwner::GuildNotice => &ui.guild_panel.notice_editor,
        EditorOwner::GuildRank => &ui.guild_panel.rank_editor,
        EditorOwner::GuildRecruit => &ui.guild_panel.recruit_editor,
    };
    let Some(editor) = model.display_editor() else {
        return;
    };
    let Some((tag, node, transform)) = texts
        .iter()
        .find(|(tag, _, _)| tag.revision == revision && tag.text == editor.text())
    else {
        return;
    };
    let caret = model
        .text_layout
        .lines
        .iter()
        .find_map(|line| {
            line.stops
                .iter()
                .find(|s| s.byte == editor.caret())
                .map(|s| Vec2::new(s.x, line.y + line.height))
        })
        .unwrap_or(Vec2::new(0., 14.));
    let scale = node.inverse_scale_factor().max(0.001);
    let point = transform.transform_point2(-node.size() * 0.5 + caret / scale);
    if let Ok(mut window) = windows.single_mut() {
        window.ime_position = point / window.scale_factor();
    }
    let _ = tag;
}

#[cfg(test)]
#[path = "text_input_tests.rs"]
mod tests;
