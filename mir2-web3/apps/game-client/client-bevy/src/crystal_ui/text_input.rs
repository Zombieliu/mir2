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
    GameShop,
    GameShopGift,
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
    if ui.shop_open() && ui.game_shop_dialog.gift.is_some() {
        return (ui.game_shop_dialog.gift_recipient.is_none()
            && ui.game_shop_dialog.gift_input.editor_focused)
            .then_some(EditorOwner::GameShopGift);
    }
    if ui.keyboard.open
        || ui.hero.modal()
        || ui.skill_assign.open
        || ui.trade_dialog.message.is_some()
        || ui.non_friend_amount_modal_open()
    {
        return None;
    }
    if ui.shop_open()
        && ui.game_shop_dialog.search_focused
        && ui.game_shop_dialog.confirmation.is_none()
        && ui.game_shop_dialog.search_input.editor_focused
    {
        return Some(EditorOwner::GameShop);
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
        EditorOwner::GameShop => &mut ui.game_shop_dialog.search_input,
        EditorOwner::GameShopGift => &mut ui.game_shop_dialog.gift_input,
        EditorOwner::Friend => &mut ui.friends,
        EditorOwner::Creature => &mut ui.creature.text_input,
        EditorOwner::Bond => &mut ui.social_bonds.input,
        EditorOwner::Group => &mut ui.group_dialog.editor,
        EditorOwner::GuildNotice => &mut ui.guild_panel.notice_editor,
        EditorOwner::GuildRank => &mut ui.guild_panel.rank_editor,
        EditorOwner::GuildRecruit => &mut ui.guild_panel.recruit_editor,
    }
}

fn editor_ref(ui: &NativePlayerUiState, owner: EditorOwner) -> &friend_dialog::FriendDialogUi {
    match owner {
        EditorOwner::GameShop => &ui.game_shop_dialog.search_input,
        EditorOwner::GameShopGift => &ui.game_shop_dialog.gift_input,
        EditorOwner::Friend => &ui.friends,
        EditorOwner::Creature => &ui.creature.text_input,
        EditorOwner::Bond => &ui.social_bonds.input,
        EditorOwner::Group => &ui.group_dialog.editor,
        EditorOwner::GuildNotice => &ui.guild_panel.notice_editor,
        EditorOwner::GuildRank => &ui.guild_panel.rank_editor,
        EditorOwner::GuildRecruit => &ui.guild_panel.recruit_editor,
    }
}

pub fn sync_draft(ui: &mut NativePlayerUiState, owner: EditorOwner) {
    match owner {
        EditorOwner::GameShop => {
            if ui.game_shop_dialog.sync_search_draft() {
                ui.game_shop_page = 0;
            }
        }
        EditorOwner::GameShopGift => {}
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
    mail_lease: Option<super::mail_text_adapter::MailTextTarget>,
    // Chat owns a plain authoritative String rather than a dialog editor.
    // The session-bound lease prevents commits from a previous screen/owner.
    chat_lease: Option<u64>,
    chat_composition: Option<Composition>,
    chat_scroll: Option<(String, f32)>,
    enabled: bool,
}

impl ImeState {
    pub fn chat_composition(&self) -> Option<&Composition> {
        self.chat_composition.as_ref()
    }

    pub fn chat_text_scroll(&self, display: &str) -> f32 {
        self.chat_scroll
            .as_ref()
            .filter(|(text, _)| text == display)
            .map_or(0.0, |(_, scroll)| *scroll)
    }

    fn chat_caret_byte(&self, draft: &str) -> usize {
        draft.len()
            + self.chat_composition.as_ref().map_or(0, |composition| {
                let mut byte = composition
                    .cursor
                    .map_or(composition.value.len(), |cursor| cursor.0)
                    .min(composition.value.len());
                while !composition.value.is_char_boundary(byte) {
                    byte -= 1;
                }
                byte
            })
    }
}

fn append_chat_commit(draft: &mut String, value: &str) {
    // Match the existing chat keyboard path: sixty Unicode scalar values,
    // excluding control characters. Preedit never enters this string.
    let available = 60usize.saturating_sub(draft.chars().count());
    draft.extend(
        value
            .chars()
            .filter(|character| !character.is_control())
            .take(available),
    );
}

pub fn process_ime(
    mut ui: ResMut<NativePlayerUiState>,
    shell: Res<NativeShellModel>,
    mut state: ResMut<ImeState>,
    mut events: MessageReader<Ime>,
    mut windows: Query<(Entity, &mut Window), With<PrimaryWindow>>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    compose: Option<Res<MailComposeUi>>,
    mut mail_editor: Option<ResMut<super::mail_editor::MailLetterEditor>>,
    pending: Option<Res<crate::pending_operations::PendingOperations>>,
    reset: Option<Res<crate::pending_operations::SessionResetRevision>>,
) {
    if ui.ime_frame_consumed {
        ui.ime_frame_consumed = false;
    }
    let events: Vec<_> = events.read().cloned().collect();
    ui.friends.sync_editor();
    ui.creature.sync_input_editor();
    ui.social_bonds.sync_editor();
    ui.game_shop_dialog.sync_search_editor();
    ui.game_shop_dialog.sync_gift_editor();
    let Ok((window_id, mut window)) = windows.single_mut() else {
        return;
    };
    let session_revision = reset.as_deref().map_or(0, |revision| revision.0);
    let mail_target = (window.focused && shell.screen == NativeShellScreen::InGame)
        .then(|| {
            Some(super::mail_text_adapter::active_mail_text_target(
                &shell,
                &ui,
                compose.as_deref()?,
                mail_editor.as_deref()?,
                pending.as_deref()?,
                session_revision,
            ))
        })
        .flatten()
        .flatten();
    let chat_target = (window.focused
        && shell.screen == NativeShellScreen::InGame
        && ui.chat_accepts_text_input()
        && editor_owner(&ui).is_none()
        && mail_target.is_none())
    .then_some(session_revision);
    if let Some(target) = chat_target {
        if let Some((old, _)) = state.lease {
            state.lease = None;
            let editor = editor_mut(&mut ui, old);
            editor.composition = None;
            editor.modifiers = [false; 4];
        }
        if state.mail_lease.is_some() {
            state.mail_lease = None;
            if let Some(editor) = mail_editor.as_deref_mut() {
                editor.clear_composition();
            }
        }
        if state.chat_lease != Some(target) {
            state.chat_lease = Some(target);
            if state.chat_composition.is_some() {
                state.chat_composition = None;
            }
            if state.enabled {
                state.enabled = false;
            }
            if window.ime_enabled {
                window.ime_enabled = false;
            }
            ui.ime_frame_consumed = events
                .iter()
                .any(|event| matches!(event, Ime::Preedit { .. } | Ime::Commit { .. }));
            return;
        }
        if !window.ime_enabled {
            window.ime_enabled = true;
        }
        let had_composition = state.chat_composition.is_some();
        let clicked = mouse.as_deref().is_some_and(|mouse| {
            mouse.just_pressed(MouseButton::Left) || mouse.just_pressed(MouseButton::Right)
        });
        if had_composition
            && (clicked
                || keys
                    .as_deref()
                    .is_some_and(|keys| keys.just_pressed(KeyCode::Escape)))
        {
            if state.chat_composition.is_some() {
                state.chat_composition = None;
            }
            if state.enabled {
                state.enabled = false;
            }
            if window.ime_enabled {
                window.ime_enabled = false;
            }
            ui.ime_frame_consumed = true;
            return;
        }
        ui.ime_frame_consumed = had_composition;
        for event in events {
            match event {
                Ime::Enabled { window } if window == window_id => {
                    if !state.enabled {
                        state.enabled = true;
                    }
                }
                Ime::Disabled { window } if window == window_id => {
                    if state.enabled {
                        state.enabled = false;
                    }
                    if state.chat_composition.is_some() {
                        state.chat_composition = None;
                    }
                }
                Ime::Preedit {
                    window,
                    value,
                    cursor,
                } if window == window_id && state.enabled => {
                    let composition = (!value.is_empty()).then_some(Composition { value, cursor });
                    if state.chat_composition != composition {
                        state.chat_composition = composition;
                    }
                    ui.ime_frame_consumed = true;
                }
                Ime::Commit { window, value } if window == window_id && state.enabled => {
                    if state.chat_composition.is_some() {
                        state.chat_composition = None;
                    }
                    append_chat_commit(&mut ui.chat_draft, &value);
                    ui.ime_frame_consumed = true;
                }
                _ => {}
            }
        }
        return;
    }
    if state.chat_lease.is_some() {
        state.chat_lease = None;
        if state.chat_composition.is_some() {
            state.chat_composition = None;
        }
        if state.enabled {
            state.enabled = false;
        }
        if window.ime_enabled {
            window.ime_enabled = false;
        }
        ui.ime_frame_consumed = events
            .iter()
            .any(|event| matches!(event, Ime::Preedit { .. } | Ime::Commit { .. }));
        return;
    }
    if let Some(target) = mail_target {
        if let Some((old, _)) = state.lease {
            state.lease = None;
            let editor = editor_mut(&mut ui, old);
            editor.composition = None;
            editor.modifiers = [false; 4];
        }
        if state.mail_lease.as_ref() != Some(&target) {
            if let Some(editor) = mail_editor.as_deref_mut() {
                editor.clear_composition();
            }
            state.mail_lease = Some(target);
            if state.enabled {
                state.enabled = false;
            }
            if window.ime_enabled {
                window.ime_enabled = false;
            }
            return;
        }
        let Some(editor) = mail_editor.as_deref_mut() else {
            state.mail_lease = None;
            if state.enabled {
                state.enabled = false;
            }
            if window.ime_enabled {
                window.ime_enabled = false;
            }
            return;
        };
        if !window.ime_enabled {
            window.ime_enabled = true;
        }
        let clicked = mouse.as_deref().is_some_and(|m| {
            m.just_pressed(MouseButton::Left) || m.just_pressed(MouseButton::Right)
        });
        let had_composition = editor.composition().is_some();
        if had_composition
            && (clicked
                || keys
                    .as_deref()
                    .is_some_and(|k| k.just_pressed(KeyCode::Escape)))
        {
            editor.clear_composition();
            if window.ime_enabled {
                window.ime_enabled = false;
            }
            if state.enabled {
                state.enabled = false;
            }
            ui.ime_frame_consumed = true;
            return;
        }
        ui.ime_frame_consumed = had_composition;
        for event in events {
            match event {
                Ime::Enabled { window } if window == window_id => {
                    if !state.enabled {
                        state.enabled = true;
                    }
                }
                Ime::Disabled { window } if window == window_id => {
                    if state.enabled {
                        state.enabled = false;
                    }
                    editor.clear_composition();
                }
                Ime::Preedit {
                    window,
                    value,
                    cursor,
                } if window == window_id && state.enabled => {
                    editor.set_composition(value, cursor);
                    ui.ime_frame_consumed = true;
                }
                Ime::Commit { window, value } if window == window_id && state.enabled => {
                    editor.clear_composition();
                    if let Some(draft) = ui.core.mail_compose.as_mut() {
                        editor.paste(draft, &value);
                    }
                    ui.ime_frame_consumed = true;
                }
                _ => {}
            }
        }
        return;
    }
    if state.mail_lease.is_some() {
        state.mail_lease = None;
        if let Some(editor) = mail_editor.as_deref_mut() {
            editor.clear_composition();
        }
        if state.enabled {
            state.enabled = false;
        }
        if window.ime_enabled {
            window.ime_enabled = false;
        }
        return;
    }
    let owner = (window.focused && shell.screen == NativeShellScreen::InGame)
        .then(|| editor_owner(&ui))
        .flatten();
    let lease = owner.map(|owner| (owner, editor_ref(&ui, owner).editor_revision));
    if state.lease != lease {
        if let Some((old, _)) = state.lease {
            let editor = editor_mut(&mut ui, old);
            editor.composition = None;
            editor.modifiers = [false; 4];
        }
        state.lease = lease;
        if state.enabled {
            state.enabled = false;
        }
        if window.ime_enabled {
            window.ime_enabled = false;
        }
        // Disable for one update before a new owner is enabled. Drop late prior-owner commits.
        return;
    }
    let Some(owner) = owner else {
        if window.ime_enabled {
            window.ime_enabled = false;
        }
        if state.enabled {
            state.enabled = false;
        }
        return;
    };
    if !window.ime_enabled {
        window.ime_enabled = true;
    }
    // Clicking while composing cancels that uncommitted presentation before focus can move.
    let clicked = mouse
        .as_deref()
        .is_some_and(|m| m.just_pressed(MouseButton::Left) || m.just_pressed(MouseButton::Right));
    let had_composition = editor_ref(&ui, owner).composition.is_some();
    if had_composition
        && (clicked
            || keys
                .as_deref()
                .is_some_and(|k| k.just_pressed(KeyCode::Escape)))
    {
        editor_mut(&mut ui, owner).composition = None;
        if window.ime_enabled {
            window.ime_enabled = false;
        }
        if state.enabled {
            state.enabled = false;
        }
        ui.ime_frame_consumed = true;
        return;
    }
    ui.ime_frame_consumed = had_composition;
    for event in events {
        match event {
            Ime::Enabled { window } if window == window_id => {
                if !state.enabled {
                    state.enabled = true;
                }
            }
            Ime::Disabled { window } if window == window_id => {
                if state.enabled {
                    state.enabled = false;
                }
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
fn shaped_chat_caret(
    text: &str,
    byte: usize,
    block: &bevy::text::ComputedTextBlock,
    info: &bevy::text::TextLayoutInfo,
) -> Vec2 {
    let scale = info.scale_factor.max(0.001);
    let mut result = Vec2::new(0.0, 13.0);
    for line in block.buffer().lines() {
        let metrics = line.metrics();
        if byte < line.text_range().start || byte > line.text_range().end {
            continue;
        }
        result = Vec2::new(metrics.offset / scale, metrics.block_max_coord / scale);
        for run in line.runs() {
            for cluster in run.visual_clusters() {
                let range = cluster.text_range();
                if byte < range.start || byte > range.end {
                    continue;
                }
                let Some(x) = cluster.visual_offset() else {
                    continue;
                };
                let start = range.start.min(text.len());
                let end = range.end.min(text.len());
                let stops: Vec<_> = text
                    .char_indices()
                    .map(|(index, _)| index)
                    .chain(std::iter::once(text.len()))
                    .filter(|index| *index >= start && *index <= end)
                    .collect();
                let position = stops.iter().position(|index| *index == byte).unwrap_or(0);
                let fraction = position as f32 / stops.len().saturating_sub(1).max(1) as f32;
                let fraction = if cluster.is_rtl() {
                    1.0 - fraction
                } else {
                    fraction
                };
                result.x = (x + cluster.advance() * fraction) / scale;
                return result;
            }
        }
    }
    result
}

fn clipped_chat_caret(caret: Vec2, viewport: Vec2) -> (Vec2, f32) {
    let max_x = (viewport.x - 1.0).max(0.0);
    let scroll = (caret.x - max_x).max(0.0);
    (
        Vec2::new(
            (caret.x - scroll).clamp(0.0, max_x),
            caret.y.clamp(0.0, viewport.y.max(0.0)),
        ),
        scroll,
    )
}

pub fn position_ime(
    ui: Res<NativePlayerUiState>,
    shell: Res<NativeShellModel>,
    mut state: ResMut<ImeState>,
    compose: Option<Res<MailComposeUi>>,
    mail_editor: Option<Res<super::mail_editor::MailLetterEditor>>,
    pending: Option<Res<crate::pending_operations::PendingOperations>>,
    reset: Option<Res<crate::pending_operations::SessionResetRevision>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    texts: Query<(
        &friend_dialog::view::FriendEditText,
        &ComputedNode,
        &bevy::ui::UiGlobalTransform,
    )>,
    mail_texts: Query<(
        &super::mail_editor::MailLetterEditText,
        &ComputedNode,
        &bevy::ui::UiGlobalTransform,
    )>,
    mut chat_texts: Query<
        (
            &Text,
            &ComputedNode,
            &bevy::ui::UiGlobalTransform,
            &bevy::text::ComputedTextBlock,
            &bevy::text::TextLayoutInfo,
            &mut Node,
        ),
        With<crate::crystal_ui::chat::CrystalChatInput>,
    >,
    chat_viewports: Query<
        (&ComputedNode, &bevy::ui::UiGlobalTransform),
        With<crate::crystal_ui::chat::CrystalChatInputViewport>,
    >,
) {
    let session_revision = reset.as_deref().map_or(0, |revision| revision.0);
    if state.chat_lease == Some(session_revision)
        && shell.screen == NativeShellScreen::InGame
        && ui.chat_accepts_text_input()
        && editor_owner(&ui).is_none()
    {
        let Ok(mut window) = windows.single_mut() else {
            return;
        };
        if !window.focused {
            return;
        }
        let mut display = ui.chat_draft.clone();
        if let Some(composition) = state.chat_composition() {
            display.push_str(&composition.value);
        }
        let Some((text, node, transform, block, info, mut style)) = chat_texts.iter_mut().next()
        else {
            return;
        };
        if text.0 != display {
            return;
        }
        let caret = shaped_chat_caret(&display, state.chat_caret_byte(&ui.chat_draft), block, info);
        let viewport = chat_viewports.iter().next();
        let (node, transform) = viewport.unwrap_or((node, transform));
        let inverse_scale = node.inverse_scale_factor().max(0.001);
        let (caret, scroll) = clipped_chat_caret(caret, node.size() * inverse_scale);
        if viewport.is_some()
            && state
                .chat_scroll
                .as_ref()
                .is_none_or(|(text, x)| text != &display || *x != scroll)
        {
            state.chat_scroll = Some((display, scroll));
        }
        if viewport.is_some() && style.left != Val::Px(-scroll) {
            style.left = Val::Px(-scroll);
        }
        let point = transform.transform_point2(-node.size() * 0.5 + caret / inverse_scale);
        let position = point / window.scale_factor();
        if window.ime_position != position {
            window.ime_position = position;
        }
        return;
    }
    let mail_target = match (
        compose.as_deref(),
        mail_editor.as_deref(),
        pending.as_deref(),
    ) {
        (Some(compose), Some(editor), Some(pending)) => {
            super::mail_text_adapter::active_mail_text_target(
                &shell,
                &ui,
                compose,
                editor,
                pending,
                session_revision,
            )
        }
        _ => None,
    };
    if let Some(target) = state.mail_lease.as_ref() {
        if Some(target) != mail_target.as_ref() {
            return;
        }
        let Some(editor) = mail_editor.as_deref() else {
            return;
        };
        let Some((tag, node, transform)) = mail_texts
            .iter()
            .find(|(tag, _, _)| tag.revision == target.editor_revision)
        else {
            return;
        };
        let caret = editor.ime_caret().unwrap_or(Vec2::new(0., 14.));
        let scale = node.inverse_scale_factor().max(0.001);
        let point = transform.transform_point2(-node.size() * 0.5 + caret / scale);
        if let Ok(mut window) = windows.single_mut() {
            let position = point / window.scale_factor();
            if window.ime_position != position {
                window.ime_position = position;
            }
        }
        let _ = tag;
        return;
    }

    let Some((owner, revision)) = state.lease else {
        return;
    };
    if editor_owner(&ui) != Some(owner) {
        return;
    }
    let model = match owner {
        EditorOwner::GameShop => &ui.game_shop_dialog.search_input,
        EditorOwner::GameShopGift => &ui.game_shop_dialog.gift_input,
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
        let position = point / window.scale_factor();
        if window.ime_position != position {
            window.ime_position = position;
        }
    }
    let _ = tag;
}

#[cfg(test)]
mod reference_chat_ime_tests {
    use super::*;

    #[derive(Resource, Default)]
    struct IdleChanges {
        ime: usize,
        window: usize,
    }

    fn observe_changes(
        ime: Res<ImeState>,
        windows: Query<Ref<Window>, With<PrimaryWindow>>,
        mut changes: ResMut<IdleChanges>,
    ) {
        if ime.is_changed() {
            changes.ime += 1;
        }
        if windows.iter().any(|window| window.is_changed()) {
            changes.window += 1;
        }
    }

    fn track_idle_changes(app: &mut App) -> (usize, usize) {
        app.init_resource::<IdleChanges>()
            .add_systems(Update, observe_changes.after(process_ime));
        app.update();
        let changes = app.world().resource::<IdleChanges>();
        (changes.ime, changes.window)
    }

    fn assert_idle_changes(app: &mut App, expected: (usize, usize)) {
        for _ in 0..3 {
            app.update();
        }
        let changes = app.world().resource::<IdleChanges>();
        assert_eq!((changes.ime, changes.window), expected);
    }

    #[test]
    fn idle_chat_and_duplicate_ime_events_do_not_dirty_ime_or_window() {
        let (mut app, window) = chat_app();
        app.world_mut().write_message(Ime::Enabled { window });
        app.update();
        let initial = track_idle_changes(&mut app);
        assert_idle_changes(&mut app, initial);
        app.world_mut().write_message(Ime::Enabled { window });
        app.update();
        assert_idle_changes(&mut app, initial);
        app.world_mut().write_message(Ime::Preedit {
            window,
            value: "治疗".into(),
            cursor: Some((3, 3)),
        });
        app.update();
        let composed = (
            app.world().resource::<IdleChanges>().ime,
            app.world().resource::<IdleChanges>().window,
        );
        assert_eq!(composed.0, initial.0 + 1);
        app.world_mut().write_message(Ime::Preedit {
            window,
            value: "治疗".into(),
            cursor: Some((3, 3)),
        });
        app.update();
        assert_idle_changes(&mut app, composed);
        assert!(
            app.world()
                .resource::<NativePlayerUiState>()
                .ime_frame_consumed,
            "composition still blocks keyboard delivery on every composing frame"
        );
        app.world_mut().write_message(Ime::Preedit {
            window,
            value: "治疗".into(),
            cursor: Some((6, 6)),
        });
        app.update();
        assert_eq!(app.world().resource::<IdleChanges>().ime, composed.0 + 1);
        app.world_mut().write_message(Ime::Commit {
            window,
            value: "治疗".into(),
        });
        app.update();
        assert_eq!(
            app.world().resource::<NativePlayerUiState>().chat_draft,
            "治疗"
        );
    }

    #[test]
    fn idle_without_owner_and_with_friend_owner_preserves_ime_change_tick() {
        let (mut app, window) = chat_app();
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .core
            .chat_focused = false;
        app.update();
        let initial = track_idle_changes(&mut app);
        assert_idle_changes(&mut app, initial);
        {
            let mut ui = app.world_mut().resource_mut::<NativePlayerUiState>();
            ui.friends.open = true;
            ui.friends.modal = Some(friend_dialog::FriendModal::Add {
                blocked: false,
                text: "A".into(),
            });
        }
        app.update();
        app.update();
        app.world_mut().write_message(Ime::Enabled { window });
        app.update();
        let friend = (
            app.world().resource::<IdleChanges>().ime,
            app.world().resource::<IdleChanges>().window,
        );
        assert_idle_changes(&mut app, friend);
        app.world_mut().write_message(Ime::Commit {
            window,
            value: "友".into(),
        });
        app.update();
        assert!(
            matches!(&app.world().resource::<NativePlayerUiState>().friends.modal,
            Some(friend_dialog::FriendModal::Add { text, .. }) if text == "A友")
        );
    }

    fn chat_app() -> (App, Entity) {
        let mut app = App::new();
        app.init_resource::<NativePlayerUiState>()
            .init_resource::<ImeState>()
            .init_resource::<crate::pending_operations::SessionResetRevision>()
            .insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..Default::default()
            })
            .add_message::<Ime>()
            .add_systems(Update, process_ime);
        let window = app
            .world_mut()
            .spawn((
                Window {
                    focused: true,
                    ..Default::default()
                },
                PrimaryWindow,
            ))
            .id();
        let mut ui = app.world_mut().resource_mut::<NativePlayerUiState>();
        ui.core.screen = mir2_ui_core::state::UiScreen::InGame;
        ui.set_chat_focused(true);
        drop(ui);
        app.update(); // acquire Chat; discard a previous owner's late events
        (app, window)
    }

    #[test]
    fn chat_preedit_is_presentation_and_commit_is_bounded_without_sending() {
        let (mut app, window) = chat_app();
        app.world_mut().write_message(Ime::Enabled { window });
        app.world_mut().write_message(Ime::Preedit {
            window,
            value: "治疗".into(),
            cursor: Some((3, 3)),
        });
        app.update();
        assert_eq!(app.world().resource::<NativePlayerUiState>().chat_draft, "");
        assert_eq!(
            app.world()
                .resource::<ImeState>()
                .chat_composition()
                .unwrap()
                .value,
            "治疗"
        );
        assert!(
            app.world()
                .resource::<NativePlayerUiState>()
                .ime_frame_consumed
        );
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .chat_draft = "药".repeat(58);
        app.world_mut().write_message(Ime::Commit {
            window,
            value: "剂\n你好".into(),
        });
        app.update();
        let ui = app.world().resource::<NativePlayerUiState>();
        assert_eq!(ui.chat_draft.chars().count(), 60);
        assert!(ui.chat_draft.ends_with("剂你"));
        assert!(ui.core.chat_focused());
        assert!(ui.ime_frame_consumed);
        assert!(app
            .world()
            .resource::<ImeState>()
            .chat_composition()
            .is_none());
    }

    #[test]
    fn chat_discards_commits_on_modal_focus_loss_and_session_change() {
        let (mut app, window) = chat_app();
        app.world_mut().write_message(Ime::Enabled { window });
        app.world_mut().write_message(Ime::Preedit {
            window,
            value: "过期".into(),
            cursor: None,
        });
        app.update();
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .keyboard
            .open = true;
        app.world_mut().write_message(Ime::Commit {
            window,
            value: "不能进入草稿".into(),
        });
        app.update();
        assert!(app
            .world()
            .resource::<NativePlayerUiState>()
            .chat_draft
            .is_empty());
        assert!(app
            .world()
            .resource::<ImeState>()
            .chat_composition()
            .is_none());
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .keyboard
            .open = false;
        app.update();
        app.world_mut().write_message(Ime::Enabled { window });
        app.update();
        app.world_mut()
            .resource_mut::<crate::pending_operations::SessionResetRevision>()
            .0 += 1;
        app.world_mut().write_message(Ime::Commit {
            window,
            value: "上一会话".into(),
        });
        app.update();
        assert!(app
            .world()
            .resource::<NativePlayerUiState>()
            .chat_draft
            .is_empty());
        app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
        app.world_mut().write_message(Ime::Commit {
            window,
            value: "失焦提交".into(),
        });
        app.update();
        assert!(app
            .world()
            .resource::<NativePlayerUiState>()
            .chat_draft
            .is_empty());
        assert!(!app.world().get::<Window>(window).unwrap().ime_enabled);
    }

    #[test]
    fn chat_caret_uses_utf8_boundary_and_clips_to_viewport() {
        let state = ImeState {
            chat_composition: Some(Composition {
                value: "治疗".into(),
                cursor: Some((4, 4)),
            }),
            ..Default::default()
        };
        assert_eq!(state.chat_caret_byte("a"), 4);
        let (caret, scroll) = clipped_chat_caret(Vec2::new(640.0, 25.0), Vec2::new(194.0, 18.0));
        assert_eq!(caret, Vec2::new(193.0, 18.0));
        assert_eq!(scroll, 447.0);
    }
}

#[cfg(test)]
#[path = "text_input_tests.rs"]
mod tests;
