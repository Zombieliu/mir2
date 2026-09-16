use super::super::*;
use super::*;

#[derive(Default, Resource)]
pub struct FriendHost {
    pub dimensions: std::collections::HashMap<(String, u16), Vec2>,
    drag: Option<(bool, Vec2)>,
    pub notice: Option<String>,
    selecting: bool,
    hover_memo: Option<(Vec2, String)>,
    hover_size: Vec2,
}
impl FriendHost {
    fn size(&self, lib: &str, index: u16) -> Option<Vec2> {
        self.dimensions.get(&(lib.into(), index)).copied()
    }
}

pub fn dispatch(
    state: &mut NativePlayerUiState,
    intents: &mut NativePlayerUiIntentQueue,
    effect: FriendEffect,
) -> bool {
    match effect {
        FriendEffect::Packet(packet) => {
            let intent = match packet {
                ClientPacket::RefreshFriends => NativePlayerUiIntent::RefreshFriends,
                ClientPacket::AddFriend { name, blocked } => {
                    NativePlayerUiIntent::AddFriend { name, blocked }
                }
                ClientPacket::RemoveFriend { character_index } => {
                    NativePlayerUiIntent::RemoveFriend { character_index }
                }
                ClientPacket::AddMemo {
                    character_index,
                    memo,
                } => NativePlayerUiIntent::AddFriendMemo {
                    character_index,
                    memo,
                },
                _ => return false,
            };
            intents.push_intent(intent)
        }
        FriendEffect::ComposeMail(recipient) => {
            state.apply(mir2_ui_core::action::UiAction::OpenMailCompose);
            state.apply(mir2_ui_core::action::UiAction::SetMailRecipient { recipient });
            true
        }
        FriendEffect::Whisper(text) => {
            state.chat_draft = text;
            state.set_chat_focused(true);
            true
        }
        FriendEffect::Offline => false,
    }
}

pub fn toggle(state: &mut NativePlayerUiState, intents: &mut NativePlayerUiIntentQueue) {
    if state.friends.open {
        state.friends.hide();
    } else if let Some(effect) = state.friends.show() {
        if !dispatch(state, intents, effect) {
            state.friends.hide();
        }
    }
}

pub fn keyboard(
    state: &mut NativePlayerUiState,
    intents: &mut NativePlayerUiIntentQueue,
    events: &[KeyboardInput],
) -> bool {
    if state.friends.modal.is_none() {
        return false;
    }
    state.friends.input_consumed = true;
    state.friends.sync_editor();
    for event in events {
        let pressed = event.state == ButtonState::Pressed;
        let mods = &mut state.friends.modifiers;
        match event.key_code {
            KeyCode::ControlLeft => mods[0] = pressed,
            KeyCode::ControlRight => mods[1] = pressed,
            KeyCode::ShiftLeft => mods[2] = pressed,
            KeyCode::ShiftRight => mods[3] = pressed,
            _ => {}
        }
        if !pressed {
            continue;
        }
        if event.key_code == KeyCode::Escape {
            state.friends.cancel_modal();
            break;
        }
        if matches!(event.key_code, KeyCode::Enter | KeyCode::NumpadEnter)
            && !matches!(state.friends.modal, Some(FriendModal::Memo { .. }))
        {
            if !event.repeat {
                submit(state, intents);
            }
            break;
        }
        if !state.friends.editor_focused {
            continue;
        }
        edit_key(&mut state.friends, event);
    }
    true
}
/// Shared physical editing operation. The owner handles Enter submission and Escape.
pub fn edit_key(f: &mut FriendDialogUi, event: &KeyboardInput) {
    let pressed = event.state == ButtonState::Pressed;
    match event.key_code {
        KeyCode::ControlLeft => f.modifiers[0] = pressed,
        KeyCode::ControlRight => f.modifiers[1] = pressed,
        KeyCode::ShiftLeft => f.modifiers[2] = pressed,
        KeyCode::ShiftRight => f.modifiers[3] = pressed,
        _ => {}
    }
    if !pressed || !f.editor_focused {
        return;
    }
    let ctrl = f.modifiers[0] || f.modifiers[1];
    let shift = f.modifiers[2] || f.modifiers[3];
    let Some(editor) = f.editor.as_mut() else {
        return;
    };
    let mut result = text_editor::EditResult::Unchanged;
    match event.key_code {
        KeyCode::KeyA if ctrl => editor.select_all(),
        KeyCode::KeyC | KeyCode::KeyX | KeyCode::KeyV if ctrl => {} // OS clipboard adapter owns these.
        KeyCode::Backspace => result = editor.delete(true),
        KeyCode::Delete => result = editor.delete(false),
        KeyCode::ArrowLeft | KeyCode::ArrowRight => {
            editor.horizontal(event.key_code == KeyCode::ArrowRight, shift);
            f.preferred_x = None;
        }
        KeyCode::Home | KeyCode::End => {
            let end = event.key_code == KeyCode::End;
            if !ctrl && f.layout_text == editor.text() {
                if let Some(byte) = f.text_layout.line_edge(f.visual_line, end) {
                    editor.set_caret(byte, shift);
                } else {
                    editor.home_end(end, ctrl, shift);
                }
            } else {
                editor.home_end(end, ctrl, shift);
            }
            f.preferred_x = None;
        }
        KeyCode::ArrowUp | KeyCode::ArrowDown if f.layout_text == editor.text() => {
            let x = *f.preferred_x.get_or_insert_with(|| {
                f.text_layout
                    .lines
                    .get(f.visual_line)
                    .and_then(|l| l.stops.iter().find(|s| s.byte == editor.caret()))
                    .map_or(0., |s| s.x)
            });
            if let Some((line, byte)) =
                f.text_layout
                    .vertical(f.visual_line, event.key_code == KeyCode::ArrowDown, x)
            {
                f.visual_line = line;
                editor.set_caret(byte, shift);
            }
        }
        KeyCode::Enter | KeyCode::NumpadEnter => result = editor.newline(),
        _ if !ctrl => {
            if let bevy::input::keyboard::Key::Character(chars) = &event.logical_key {
                result = editor.insert_with_policy(chars, text_editor::InsertPolicy::FitPrefix);
            }
        }
        _ => {}
    }
    f.commit_editor(result);
}

fn submit(state: &mut NativePlayerUiState, intents: &mut NativePlayerUiIntentQueue) {
    if let Some(packet) = state.friends.modal_packet() {
        if dispatch(state, intents, FriendEffect::Packet(packet)) {
            state.friends.cancel_modal();
        } else {
            state.friends.edit_notice = Some("Unable to send. Please try again.".into());
        }
    } else {
        state.friends.edit_notice = Some("Enter a valid name or memo before saving.".into());
    }
}

pub(in super::super) fn process(
    mut state: ResMut<NativePlayerUiState>,
    mut host: ResMut<FriendHost>,
    shell: Option<Res<NativeShellModel>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut intents: ResMut<NativePlayerUiIntentQueue>,
    mut chat: Option<ResMut<crate::chat::ChatModel>>,
) {
    state.friends.input_consumed = state.friends.modal.is_some();
    state.friends.sync_editor();
    host.hover_memo = None;
    if let Some(text) = host.notice.take() {
        if let Some(chat) = chat.as_deref_mut() {
            chat.push(crate::chat::ChatLine {
                text,
                channel: "system".into(),
            });
        }
    }
    if shell.is_none_or(|s| s.screen != NativeShellScreen::InGame) {
        host.drag = None;
        host.selecting = false;
        state.friends.modifiers = [false; 4];
        return;
    }
    if !state.friends.open
        || state.keyboard.open
        || state.non_friend_amount_modal_open()
        || state.trade_dialog.message.is_some()
    {
        host.drag = None;
        host.selecting = false;
        state.friends.modifiers = [false; 4];
        return;
    }
    let (Ok(window), Some(mouse)) = (windows.single(), mouse) else {
        return;
    };
    if !window.focused {
        host.drag = None;
        host.selecting = false;
        state.friends.modifiers = [false; 4];
        return;
    }
    let Some(cursor) = help_cursor_logical(window) else {
        return;
    };
    if state.friends.modal.is_some() {
        if mouse.just_pressed(MouseButton::Left) {
            if matches!(state.friends.modal, Some(FriendModal::Memo { .. })) {
                if let Some(position) = state.friends.memo_position {
                    let local = cursor - Vec2::from(position);
                    state.friends.editor_focused = false;
                    match view::memo_hit_action(local, |l, i| host.size(l, i)) {
                        Some(view::MemoAction::Save) => submit(&mut state, &mut intents),
                        Some(view::MemoAction::Cancel) => state.friends.cancel_modal(),
                        Some(view::MemoAction::Edit) => {
                            focus_editor(&mut state.friends, local - Vec2::new(15., 30.), false);
                            host.selecting = true;
                        }
                        None if local.y >= 0.
                            && local.y < 25.
                            && local.x >= 0.
                            && local.x < 168. =>
                        {
                            host.drag = Some((true, local))
                        }
                        _ => {}
                    }
                }
            } else {
                let (frame, yes, no, y) =
                    if matches!(state.friends.modal, Some(FriendModal::Add { .. })) {
                        (660, 60., 160., 123.)
                    } else {
                        (360, 260., 360., 157.)
                    };
                if let Some(size) = host.size("Prguse", frame) {
                    let local = cursor - (Vec2::new(1024., 768.) - size) / 2.;
                    if frame == 660
                        && local.x >= 23.
                        && local.x < 263.
                        && local.y >= 86.
                        && local.y < 105.
                    {
                        focus_editor(&mut state.friends, local - Vec2::new(23., 86.), false);
                        host.selecting = true;
                    } else {
                        state.friends.editor_focused = false;
                    }
                    if local.y >= y && local.y < y + 25. {
                        if local.x >= yes && local.x < yes + 76. {
                            submit(&mut state, &mut intents);
                        } else if local.x >= no && local.x < no + 76. {
                            state.friends.cancel_modal();
                        }
                    }
                }
            }
        }
    } else if mouse.just_pressed(MouseButton::Left) {
        if let Some(position) = state.friends.position {
            let local = cursor - Vec2::from(position);
            if let Some(action) = view::hit_action(&state.friends, local, |l, i| host.size(l, i)) {
                if let Some(effect) = state.friends.action(action) {
                    if effect == FriendEffect::Offline {
                        host.notice = Some("Player is not online.".into());
                    } else {
                        dispatch(&mut state, &mut intents, effect);
                    }
                }
            } else if local.y >= 0. && local.y < 30. && local.x >= 0. && local.x < 237. {
                host.drag = Some((false, local));
            }
        }
    }
    if state.friends.modal.is_none() {
        if let Some(position) = state.friends.position {
            if let Some(text) = view::memo_text_at(&state.friends, cursor - Vec2::from(position)) {
                host.hover_memo = Some((cursor, text.to_owned()));
            }
        }
    }
    if mouse.pressed(MouseButton::Left) {
        if host.selecting {
            let origin = if matches!(state.friends.modal, Some(FriendModal::Memo { .. })) {
                state
                    .friends
                    .memo_position
                    .map(|p| Vec2::from(p) + Vec2::new(15., 30.))
            } else {
                host.size("Prguse", 660)
                    .map(|s| (Vec2::new(1024., 768.) - s) / 2. + Vec2::new(23., 86.))
            };
            if let Some(origin) = origin {
                focus_editor(&mut state.friends, cursor - origin, true);
            }
        }
        if let Some((memo, offset)) = host.drag {
            let p = (cursor - offset).to_array();
            if memo {
                state.friends.memo_position = Some(p);
            } else {
                state.friends.position = Some(p);
            }
        }
    } else {
        host.drag = None;
        host.selecting = false;
    }
}

pub fn focus_editor(f: &mut FriendDialogUi, local: Vec2, extend: bool) {
    f.sync_editor();
    f.editor_focused = true;
    let point = local + Vec2::from(f.text_scroll);
    if let Some(editor) = f.editor.as_mut() {
        if f.layout_text == editor.text() && f.text_layout.valid_for(editor) {
            if let Some(byte) = f.text_layout.hit_test(point.x, point.y) {
                editor.set_caret(byte, extend);
                f.visual_line = f
                    .text_layout
                    .lines
                    .iter()
                    .position(|l| point.y >= l.y && point.y < l.y + l.height)
                    .unwrap_or(f.visual_line);
                f.preferred_x = None;
            }
        }
    }
}

pub(in super::super) fn render_system(
    mut commands: Commands,
    roots: Query<Entity, With<OverlayRoot>>,
    panels: Query<Entity, Or<(With<view::FriendPanel>, With<view::MemoPanel>)>>,
    tooltip_nodes: Query<&ComputedNode, With<view::MemoTooltip>>,
    text_blocks: Query<(
        &view::FriendEditText,
        &bevy::text::ComputedTextBlock,
        &bevy::text::TextLayoutInfo,
    )>,
    mut state: ResMut<NativePlayerUiState>,
    mut host: ResMut<FriendHost>,
    shell: Option<Res<NativeShellModel>>,
    assets: Option<Res<AssetServer>>,
    images: Option<Res<Assets<Image>>>,
    mut handles: Local<std::collections::HashMap<(String, u16), Handle<Image>>>,
) {
    state.friends.sync_editor();
    if let Some(node) = tooltip_nodes.iter().next() {
        host.hover_size = node.size() * node.inverse_scale_factor();
    }
    for (tag, block, info) in &text_blocks {
        capture_layout(&mut state.friends, tag, block, info);
    }
    for p in &panels {
        commands.entity(p).despawn();
    }
    if shell.is_none_or(|s| s.screen != NativeShellScreen::InGame) || !state.friends.open {
        return;
    }
    let (Ok(root), Some(assets), Some(images)) = (roots.single(), assets, images) else {
        return;
    };
    for (lib, ids) in [
        (
            "Title",
            vec![
                6, 163, 164, 166, 167, 199, 209, 200, 201, 202, 203, 204, 205, 206, 207, 208, 210,
                211, 212, 382, 383, 384, 385, 386, 387,
            ],
        ),
        (
            "Prguse",
            vec![
                360, 660, 554, 555, 556, 557, 558, 559, 560, 561, 562, 563, 564, 565, 566, 567, 568,
            ],
        ),
        ("Prguse2", vec![240, 241, 242, 243, 244, 245, 360, 361, 362]),
    ] {
        for i in ids {
            let handle = handles
                .entry((lib.into(), i))
                .or_insert_with(|| assets.load(format!("original-ui/{lib}/{i}.png")));
            if let Some(img) = images.get(handle) {
                host.dimensions.insert(
                    (lib.into(), i),
                    Vec2::new(img.width() as f32, img.height() as f32),
                );
            }
        }
    }
    commands.entity(root).with_children(|parent| {
        if let Some((cursor, text)) = &host.hover_memo {
            parent
                .spawn((
                    view::MemoPanel,
                    view::MemoTooltip,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px((cursor.x + 15.).min(1024. - host.hover_size.x).max(0.)),
                        top: Val::Px(cursor.y.min(768. - host.hover_size.y).max(0.)),
                        padding: UiRect::all(Val::Px(4.)),
                        border: UiRect::all(Val::Px(1.)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(50. / 255., 50. / 255., 50. / 255., 0.7)),
                    BorderColor::all(Color::srgb_u8(128, 128, 128)),
                    GlobalZIndex(1001),
                ))
                .with_children(|p| {
                    p.spawn((
                        Text::new(view::memo_hint_text(text)),
                        crate::crystal_ui::typography::crystal_text_font(32. / 3.),
                        TextColor(Color::WHITE),
                        TextLayout::new(Justify::Left, LineBreak::NoWrap),
                    ));
                });
        }
        view::render(
            parent,
            &assets,
            &mut state.friends,
            Vec2::new(1024., 768.),
            |l, i| host.size(l, i),
        );
        view::render_memo(
            parent,
            &assets,
            &mut state.friends,
            Vec2::new(1024., 768.),
            |l, i| host.size(l, i),
        );
        if let Some(modal) = &state.friends.modal {
            match modal {
                FriendModal::Add { blocked, text } => prompt(
                    parent,
                    &assets,
                    &host,
                    660,
                    if *blocked {
                        "Please enter the name of the person you would like to Block."
                    } else {
                        "Please enter the name of the person you would like to Add."
                    },
                    Some(text),
                    &state.friends,
                ),
                FriendModal::Remove { name, .. } => prompt(
                    parent,
                    &assets,
                    &host,
                    360,
                    &format!("Are you sure you want to remove {name}?"),
                    None,
                    &state.friends,
                ),
                _ => {}
            }
        }
    });
}
fn prompt(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    host: &FriendHost,
    frame: u16,
    caption: &str,
    text: Option<&str>,
    model: &FriendDialogUi,
) {
    let Some(size) = host.size("Prguse", frame) else {
        return;
    };
    let p = (Vec2::new(1024., 768.) - size) / 2.;
    parent
        .spawn((
            view::MemoPanel,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(p.x),
                top: Val::Px(p.y),
                width: Val::Px(size.x),
                height: Val::Px(size.y),
                ..default()
            },
            FocusPolicy::Block,
            GlobalZIndex(1000),
        ))
        .with_children(|panel| {
            spawn_overlay_frame(
                panel,
                assets,
                if frame == 660 {
                    "original-ui/Prguse/660.png"
                } else {
                    "original-ui/Prguse/360.png"
                },
                size.x,
                size.y,
            );
            let (x, y, w, h) = if text.is_some() {
                (25., 25., 235., 40.)
            } else {
                (35., 35., 390., 110.)
            };
            view::wrapped_text(panel, caption, CrystalRect::new(x, y, w, h), Color::WHITE);
            if text.is_some() {
                view::render_editor(panel, model, CrystalRect::new(23., 86., 240., 19.), false);
            }
            if let Some(notice) = &model.edit_notice {
                view::wrapped_text(
                    panel,
                    notice,
                    CrystalRect::new(15., size.y + 2., size.x - 30., 35.),
                    Color::srgb_u8(255, 90, 70),
                );
            }
            let controls = if text.is_some() {
                [(200, 60., 123.), (203, 160., 123.)]
            } else {
                [(206, 260., 157.), (210, 360., 157.)]
            };
            for (index, x, y) in controls {
                if let Some(d) = host.size("Title", index) {
                    let spec = CrystalButtonSpec::new(
                        "Title",
                        index,
                        index + 1,
                        index + 2,
                        CrystalRect::new(x, y, d.x, d.y),
                        d.x,
                        d.y,
                    );
                    spawn_crystal_image_button(
                        panel,
                        assets,
                        spec,
                        CrystalButtonAssetSet::from_spec(spec),
                        view::MemoAction::Save,
                        false,
                        true,
                    );
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(code: KeyCode, text: &str, pressed: bool) -> KeyboardInput {
        KeyboardInput {
            key_code: code,
            logical_key: bevy::input::keyboard::Key::Character(text.into()),
            state: if pressed {
                ButtonState::Pressed
            } else {
                ButtonState::Released
            },
            text: Some(text.into()),
            repeat: false,
            window: Entity::PLACEHOLDER,
        }
    }
    fn add_state() -> NativePlayerUiState {
        let mut state = NativePlayerUiState::default();
        state.friends.open = true;
        state.friends.action(FriendAction::Add);
        state.friends.paste("Old😀");
        state
    }
    #[test]
    fn ctrl_a_replaces_graphemes_and_shortcut_letters_are_not_inserted() {
        let mut s = add_state();
        let mut q = NativePlayerUiIntentQueue::default();
        keyboard(
            &mut s,
            &mut q,
            &[
                key(KeyCode::ControlLeft, "", true),
                key(KeyCode::KeyA, "a", true),
                key(KeyCode::KeyV, "v", true),
                key(KeyCode::ControlLeft, "", false),
                key(KeyCode::KeyN, "新", true),
            ],
        );
        assert_eq!(s.friends.editor.as_ref().unwrap().text(), "新");
    }
    #[test]
    fn unfocused_editor_does_not_append_and_full_queue_retains_draft() {
        let mut s = add_state();
        let mut q = NativePlayerUiIntentQueue::default();
        s.friends.editor_focused = false;
        keyboard(&mut s, &mut q, &[key(KeyCode::KeyX, "x", true)]);
        assert_eq!(s.friends.editor.as_ref().unwrap().text(), "Old😀");
        while q.push_intent(NativePlayerUiIntent::RefreshFriends) {}
        submit(&mut s, &mut q);
        assert!(s.friends.modal.is_some());
        assert_eq!(s.friends.editor.as_ref().unwrap().text(), "Old😀");
        assert!(s.friends.edit_notice.is_some());
    }
}

pub fn capture_layout(
    f: &mut FriendDialogUi,
    tag: &view::FriendEditText,
    block: &bevy::text::ComputedTextBlock,
    info: &bevy::text::TextLayoutInfo,
) {
    if tag.revision != f.editor_revision || f.display_editor().is_none_or(|e| e.text() != tag.text)
    {
        return;
    }
    let editor = f.display_editor().unwrap();
    let scale = info.scale_factor.max(0.001);
    let mut layout = text_editor::TextLayout::default();
    for line in block.buffer().lines() {
        let metrics = line.metrics();
        let mut stops = Vec::new();
        for run in line.runs() {
            for cluster in run.visual_clusters() {
                let Some(x) = cluster.visual_offset() else {
                    continue;
                };
                let range = cluster.text_range();
                let mut bytes: Vec<_> =
                    unicode_segmentation::UnicodeSegmentation::grapheme_indices(
                        editor.text(),
                        true,
                    )
                    .map(|(i, _)| i)
                    .chain(std::iter::once(editor.text().len()))
                    .filter(|i| *i >= range.start && *i <= range.end)
                    .collect();
                if cluster.is_rtl() {
                    bytes.reverse();
                }
                let count = bytes.len().saturating_sub(1).max(1) as f32;
                for (i, byte) in bytes.into_iter().enumerate() {
                    stops.push(text_editor::CaretStop {
                        byte,
                        x: (x + cluster.advance() * i as f32 / count) / scale,
                    });
                }
            }
        }
        if stops.is_empty() {
            stops.push(text_editor::CaretStop {
                byte: line.text_range().start.min(editor.text().len()),
                x: metrics.offset / scale,
            });
        }
        stops.sort_by(|a, b| a.x.total_cmp(&b.x));
        stops.dedup_by(|a, b| a.byte == b.byte && a.x == b.x);
        layout.lines.push(text_editor::VisualLine {
            y: metrics.block_min_coord / scale,
            height: (metrics.block_max_coord - metrics.block_min_coord).max(1.) / scale,
            stops,
        });
    }
    if layout.valid_for(&editor) {
        if layout
            .lines
            .get(f.visual_line)
            .is_none_or(|l| !l.stops.iter().any(|s| s.byte == editor.caret()))
        {
            f.visual_line = layout
                .lines
                .iter()
                .position(|l| l.stops.iter().any(|s| s.byte == editor.caret()))
                .unwrap_or(0);
        }
        if let Some(line) = layout.lines.get(f.visual_line) {
            if let Some(stop) = line.stops.iter().find(|s| s.byte == editor.caret()) {
                let h = tag.viewport[1];
                let caret_changed =
                    f.captured_caret != Some(editor.caret()) || f.layout_text != tag.text;
                if caret_changed && line.y < f.text_scroll[1] {
                    f.text_scroll[1] = line.y;
                }
                if caret_changed && line.y + line.height > f.text_scroll[1] + h {
                    f.text_scroll[1] = line.y + line.height - h;
                }
                if caret_changed && !matches!(f.modal, Some(FriendModal::Memo { .. })) {
                    f.text_scroll[0] = f.text_scroll[0]
                        .min(stop.x)
                        .max(stop.x - (tag.viewport[0] - 3.).max(1.));
                }
            }
        }
        f.captured_caret = Some(editor.caret());
        f.text_layout = layout;
        f.layout_text = tag.text.clone();
    }
}
