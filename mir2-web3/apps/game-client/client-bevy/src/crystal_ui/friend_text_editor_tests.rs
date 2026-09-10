use super::*;

#[test]
fn mir_input_box_prefix_policy_preserves_fitting_utf16_text_and_selection() {
    let mut e = FriendTextEditor::new("Old".into(), 50, false);
    e.select_all();
    assert_eq!(
        e.insert_with_policy(&"😀".repeat(26), InsertPolicy::FitPrefix),
        EditResult::Changed
    );
    assert_eq!(e.text(), "😀".repeat(25));
    assert_eq!(
        e.insert_with_policy("x", InsertPolicy::FitPrefix),
        EditResult::Unchanged
    );
    let mut memo = FriendTextEditor::new("a".repeat(200), 200, true);
    assert_eq!(
        memo.insert_with_policy("more", InsertPolicy::RejectOverflow),
        EditResult::OverBudget
    );
    assert_eq!(memo.text(), "a".repeat(200));
}

#[test]
fn deletion_and_arrows_preserve_unicode_graphemes() {
    let mut e = FriendTextEditor::new("a👩‍👩‍👧‍👦e\u{301}".into(), 200, true);
    e.delete(true);
    assert_eq!(e.text(), "a👩‍👩‍👧‍👦");
    e.horizontal(false, false);
    assert_eq!(e.caret(), 1);
    e.delete(false);
    assert_eq!(e.text(), "a");
}

#[test]
fn selection_replacement_uses_utf16_budget_atomically() {
    let mut e = FriendTextEditor::new("abc😀".into(), 5, true);
    assert_eq!(e.insert("x"), EditResult::OverBudget);
    assert_eq!(e.text(), "abc😀");
    e.horizontal(false, true);
    assert_eq!(e.selected_text(), "😀");
    assert_eq!(e.insert("xyz"), EditResult::OverBudget);
    assert_eq!(e.selected_text(), "😀");
    assert_eq!(e.insert("xy"), EditResult::Changed);
    assert_eq!(e.text(), "abcxy");
    e.select_all();
    e.delete(true);
    assert_eq!(e.text(), "");
}

#[test]
fn newline_and_paste_share_budget_and_normalization() {
    let mut e = FriendTextEditor::new("a".into(), 4, true);
    assert_eq!(e.insert("\r\nb\rc"), EditResult::OverBudget);
    assert_eq!(e.text(), "a");
    assert_eq!(e.insert("\r\nb"), EditResult::Changed);
    assert_eq!(e.newline(), EditResult::Changed);
    assert_eq!(e.newline(), EditResult::OverBudget);
    assert_eq!(e.text(), "a\nb\n");
    let mut single = FriendTextEditor::new("".into(), 50, false);
    single.insert("a\r\nb\t");
    assert_eq!(single.text(), "ab");
    assert_eq!(single.newline(), EditResult::Unchanged);
}

#[test]
fn home_end_and_selection_collapse_follow_line_boundaries() {
    let mut e = FriendTextEditor::new("abc\ndef".into(), 200, true);
    e.home_end(false, false, false);
    assert_eq!(e.caret(), 4);
    e.home_end(true, false, true);
    assert_eq!(e.selected_text(), "def");
    e.horizontal(false, false);
    assert_eq!(e.caret(), 4);
    assert!(e.selection().is_empty());
    e.home_end(false, true, false);
    assert_eq!(e.caret(), 0);
    e.home_end(true, true, true);
    assert_eq!(e.selected_text(), "abc\ndef");
}

#[test]
fn unsafe_offsets_snap_and_inserted_combining_text_keeps_valid_caret() {
    let mut e = FriendTextEditor::new("aé".into(), 200, true);
    e.set_caret(2, false);
    assert_eq!(e.caret(), 1);
    e.set_caret(999, false);
    assert_eq!(e.caret(), 3);
    e.insert("\u{301}");
    e.delete(true);
    assert_eq!(e.text(), "a");
}

#[test]
fn existing_overlong_draft_can_be_repaired_without_truncation() {
    let mut e = FriendTextEditor::new("123456".into(), 5, true);
    assert!(!e.within_budget());
    e.delete(true);
    assert!(e.within_budget());
    assert_eq!(e.text(), "12345");
}

#[test]
fn renderer_layout_supports_wrapping_hit_selection_and_vertical_navigation() {
    let e = FriendTextEditor::new("abcd".into(), 200, true);
    let l = TextLayout {
        lines: vec![
            VisualLine {
                y: 0.,
                height: 12.,
                stops: vec![
                    CaretStop { byte: 0, x: 0. },
                    CaretStop { byte: 1, x: 5. },
                    CaretStop { byte: 2, x: 15. },
                ],
            },
            VisualLine {
                y: 12.,
                height: 12.,
                stops: vec![
                    CaretStop { byte: 2, x: 0. },
                    CaretStop { byte: 3, x: 6. },
                    CaretStop { byte: 4, x: 10. },
                ],
            },
        ],
    };
    assert!(l.valid_for(&e));
    assert_eq!(l.hit_test(5., 18.), Some(3));
    assert_eq!(l.line_edge(1, false), Some(2));
    assert_eq!(l.vertical(0, true, 5.), Some((1, 3)));
    assert_eq!(
        l.selection_rects(1..3),
        vec![
            SelectionRect {
                x: 5.,
                y: 0.,
                width: 10.,
                height: 12.
            },
            SelectionRect {
                x: 0.,
                y: 12.,
                width: 6.,
                height: 12.
            },
        ]
    );
}
