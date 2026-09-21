use super::*;

fn key(code: KeyCode, text: &str) -> KeyboardInput {
    KeyboardInput {
        key_code: code,
        logical_key: bevy::input::keyboard::Key::Character(text.into()),
        state: ButtonState::Pressed,
        text: (!text.is_empty()).then_some(text.into()),
        repeat: false,
        window: Entity::PLACEHOLDER,
    }
}

fn release(code: KeyCode) -> KeyboardInput {
    KeyboardInput {
        key_code: code,
        logical_key: bevy::input::keyboard::Key::Character("".into()),
        state: ButtonState::Released,
        text: None,
        repeat: false,
        window: Entity::PLACEHOLDER,
    }
}

fn draft(message: &str) -> mir2_ui_core::state::MailComposeDraft {
    mir2_ui_core::state::MailComposeDraft {
        recipient: "Receiver".into(),
        message: message.into(),
        ..default()
    }
}

fn line(y: f32, bytes: &[usize]) -> VisualLine {
    VisualLine {
        y,
        height: 20.0,
        stops: bytes
            .iter()
            .enumerate()
            .map(|(index, byte)| CaretStop {
                byte: *byte,
                // Deliberately uneven advances: keyboard movement must use
                // captured glyph positions rather than character estimates.
                x: [0.0, 11.0, 39.0, 72.0].get(index).copied().unwrap_or(96.0),
            })
            .collect(),
    }
}

#[test]
fn unicode_selection_delete_and_ctrl_a_keep_grapheme_boundaries() {
    let mut editor = MailLetterEditor::default();
    let mut draft = draft("a👩‍👩‍👧‍👦e\u{301}z");
    editor.sync(true, Some(&draft.message));

    editor.edit_key(&key(KeyCode::Backspace, ""), &mut draft);
    assert_eq!(draft.message, "a👩‍👩‍👧‍👦e\u{301}");
    editor.edit_key(&key(KeyCode::ArrowLeft, ""), &mut draft);
    editor.edit_key(&key(KeyCode::Delete, ""), &mut draft);
    assert_eq!(draft.message, "a👩‍👩‍👧‍👦");

    editor.edit_key(&key(KeyCode::ControlLeft, ""), &mut draft);
    editor.edit_key(&key(KeyCode::KeyA, "a"), &mut draft);
    editor.edit_key(&release(KeyCode::ControlLeft), &mut draft);
    editor.edit_key(&key(KeyCode::KeyN, "新"), &mut draft);
    assert_eq!(draft.message, "新");
    assert_eq!(editor.active_editor().unwrap().selection(), "新".len().."新".len());
}

#[test]
fn home_end_arrows_and_shift_use_captured_visual_layout_and_scroll_to_caret() {
    let message = (0..12).map(|index| format!("{index}\n")).collect::<String>();
    let mut editor = MailLetterEditor::default();
    let mut draft = draft(&message);
    editor.sync(true, Some(&draft.message));
    let mut starts = message.match_indices('\n').map(|(index, _)| index + 1).collect::<Vec<_>>();
    starts.insert(0, 0);
    let mut lines = Vec::new();
    for (index, start) in starts.iter().copied().enumerate() {
        let end = message[start..]
            .find('\n')
            .map(|offset| start + offset)
            .unwrap_or(message.len());
        lines.push(line(index as f32 * 20.0, &[start, end]));
    }
    editor.install_layout(lines);

    editor.edit_key(&key(KeyCode::ControlLeft, ""), &mut draft);
    editor.edit_key(&key(KeyCode::End, ""), &mut draft);
    editor.edit_key(&release(KeyCode::ControlLeft), &mut draft);
    assert_eq!(editor.active_editor().unwrap().caret(), message.len());
    assert!(editor.scroll().y > 0.0, "bottom caret must scroll into the real viewport");

    editor.edit_key(&key(KeyCode::ArrowUp, ""), &mut draft);
    let first_up = editor.active_editor().unwrap().caret();
    editor.edit_key(&key(KeyCode::ShiftLeft, ""), &mut draft);
    editor.edit_key(&key(KeyCode::ArrowUp, ""), &mut draft);
    editor.edit_key(&release(KeyCode::ShiftLeft), &mut draft);
    assert!(editor.active_editor().unwrap().selection().start < first_up);

    editor.edit_key(&key(KeyCode::ControlLeft, ""), &mut draft);
    editor.edit_key(&key(KeyCode::Home, ""), &mut draft);
    editor.edit_key(&release(KeyCode::ControlLeft), &mut draft);
    assert_eq!(editor.active_editor().unwrap().caret(), 0);
}

#[test]
fn pointer_hit_test_uses_uneven_shaped_stops_and_selection_extends() {
    let mut editor = MailLetterEditor::default();
    let mut draft = draft("abcd");
    editor.sync(true, Some(&draft.message));
    editor.install_layout(vec![line(0.0, &[0, 1, 3, 4])]);

    editor.pointer(Vec2::new(35.0, 4.0), false);
    assert_eq!(editor.active_editor().unwrap().caret(), 3);
    editor.pointer(Vec2::new(10.0, 4.0), true);
    assert_eq!(editor.active_editor().unwrap().selection(), 1..3);
}

#[test]
fn wrapped_boundary_keeps_the_chosen_downward_line_and_scroll_affinity() {
    let mut editor = MailLetterEditor::default();
    let mut draft = draft("abc");
    editor.sync(true, Some(&draft.message));
    editor.install_layout(vec![
        VisualLine {
            y: 160.0,
            height: 20.0,
            stops: vec![
                CaretStop { byte: 0, x: 0.0 },
                CaretStop { byte: 2, x: 37.0 },
            ],
        },
        VisualLine {
            y: 180.0,
            height: 20.0,
            stops: vec![
                CaretStop { byte: 2, x: 37.0 },
                CaretStop { byte: 3, x: 58.0 },
            ],
        },
    ]);

    editor.pointer(Vec2::new(37.0, 164.0), false);
    assert_eq!(editor.visual_line, 0);
    editor.edit_key(&key(KeyCode::ArrowDown, ""), &mut draft);
    assert_eq!(editor.active_editor().unwrap().caret(), 2);
    assert_eq!(editor.visual_line, 1, "the shared wrap byte retains downward affinity");
    assert!(editor.scroll().y > 0.0, "scrolling follows the chosen second visual line");
}

#[test]
fn retained_letter_draft_keeps_selection_through_a_transient_modal() {
    let mut editor = MailLetterEditor::default();
    let mut draft = draft("letter body");
    editor.sync(true, Some(&draft.message));
    editor.install_layout(vec![line(0.0, &[0, 6, draft.message.len()])]);
    editor.pointer(Vec2::new(11.0, 4.0), false);
    editor.pointer(Vec2::new(72.0, 4.0), true);
    let selection = editor.active_editor().unwrap().selection();

    // The parent overlay leaves the editor intact while a recipient/feedback
    // layer temporarily covers the same retained Letter draft.
    editor.sync(true, Some(&draft.message));
    assert_eq!(editor.active_editor().unwrap().selection(), selection);
}

#[test]
fn wheel_uses_shaped_extent_clamps_and_preserves_caret_and_selection() {
    let message = "x\n".repeat(20);
    let mut editor = MailLetterEditor::default();
    let mut draft = draft(&message);
    editor.sync(true, Some(&draft.message));
    let mut starts = message
        .match_indices('\n')
        .map(|(index, _)| index + 1)
        .collect::<Vec<_>>();
    starts.insert(0, 0);
    editor.install_layout(
        starts
            .iter()
            .copied()
            .enumerate()
            .map(|(index, start)| {
                let end = message[start..]
                    .find('\n')
                    .map(|offset| start + offset)
                    .unwrap_or(message.len());
                line(index as f32 * 20.0, &[start, end])
            })
            .collect(),
    );
    let caret = editor.active_editor().unwrap().caret();
    let selection = editor.active_editor().unwrap().selection();

    assert!(editor.scroll_wheel_lines(-100.0));
    assert_eq!(editor.scroll().y, 259.0, "wheel clamps to shaped bottom");
    assert_eq!(editor.active_editor().unwrap().caret(), caret);
    assert_eq!(editor.active_editor().unwrap().selection(), selection);
    // Text layout capture happens every render frame. Re-capturing identical
    // shaped geometry must retain a manual wheel position instead of snapping
    // to the unchanged caret.
    editor.accept_layout(
        EditorTextLayout {
            lines: starts
                .iter()
                .copied()
                .enumerate()
                .map(|(index, start)| {
                    let end = message[start..]
                        .find('\n')
                        .map(|offset| start + offset)
                        .unwrap_or(message.len());
                    line(index as f32 * 20.0, &[start, end])
                })
                .collect(),
        },
        message.clone(),
    );
    assert_eq!(editor.scroll().y, 259.0, "unchanged frame capture keeps wheel scroll");
    assert!(!editor.scroll_wheel_lines(-1.0), "bottom cannot overscroll");

    editor.accept_layout(
        EditorTextLayout {
            lines: starts
                .iter()
                .copied()
                .enumerate()
                .map(|(index, start)| {
                    let end = message[start..]
                        .find('\n')
                        .map(|offset| start + offset)
                        .unwrap_or(message.len());
                    line(index as f32 * 10.0, &[start, end])
                })
                .collect(),
        },
        message.clone(),
    );
    assert_eq!(editor.scroll().y, 59.0, "new shaped extent clamps retained wheel scroll");

    assert!(editor.scroll_wheel_pixels(10_000.0));
    assert_eq!(editor.scroll().y, 0.0, "wheel clamps to shaped top");
}
