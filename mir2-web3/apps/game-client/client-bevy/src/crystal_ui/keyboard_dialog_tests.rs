use super::*;

fn index(model: &KeyboardDialogUi, name: &str) -> usize {
    model
        .bindings
        .iter()
        .position(|b| b.function == name)
        .unwrap()
}

#[test]
fn crystal_defaults_preserve_all_96_real_entries_and_overlapping_tab_actions() {
    let m = KeyboardDialogUi::default();
    assert_eq!(m.bindings.len(), 96);
    assert_eq!(
        m.matching_functions("Tab", KeyModifiers::default()),
        vec!["Pickup", "DropView"]
    );
    assert_eq!(
        m.matching_functions("I", KeyModifiers::default()),
        vec!["Inventory2"]
    );
    assert_eq!(
        m.matching_functions(
            "I",
            KeyModifiers {
                ctrl: true,
                ..Default::default()
            }
        ),
        vec!["HeroInventory"]
    );
    assert!(m
        .matching_functions(
            "Insert",
            KeyModifiers {
                ctrl: true,
                ..Default::default()
            }
        )
        .is_empty());
}

#[test]
fn strict_and_relaxed_capture_restore_all_modifiers_on_reset() {
    let mut m = KeyboardDialogUi::default();
    m.show();
    let i = index(&m, "Inventory");
    m.action(KeyboardAction::Bind(i));
    assert!(m.capture("ControlKey", KeyModifiers::default()));
    assert_eq!(m.waiting, Some(i));
    assert!(m.capture(
        "K",
        KeyModifiers {
            ctrl: true,
            ..Default::default()
        }
    ));
    assert_eq!(
        (
            m.bindings[i].ctrl,
            m.bindings[i].alt,
            m.bindings[i].shift,
            m.bindings[i].tilde
        ),
        (1, 0, 0, 0)
    );
    m.action(KeyboardAction::Enforce);
    m.action(KeyboardAction::Bind(i));
    m.capture(
        "Escape",
        KeyModifiers {
            alt: true,
            tilde: true,
            ..Default::default()
        },
    );
    assert_eq!(m.bindings[i].display(), "Alt + ~ + Escape");
    assert_eq!((m.bindings[i].ctrl, m.bindings[i].shift), (2, 2));
    m.action(KeyboardAction::Reset);
    assert_eq!(m.bindings, crystal_default_keybinds());
}

#[test]
fn delete_unbinds_duplicates_are_allowed_and_second_click_cancels_capture() {
    let mut m = KeyboardDialogUi::default();
    m.show();
    let i = index(&m, "Inventory");
    m.action(KeyboardAction::Bind(i));
    m.action(KeyboardAction::Bind(i + 1));
    assert_eq!(m.waiting, None);
    m.action(KeyboardAction::Bind(i));
    assert!(m
        .matching_functions("F9", KeyModifiers::default())
        .is_empty());
    m.capture("Delete", KeyModifiers::default());
    assert_eq!(m.bindings[i].key, "None");
    assert_eq!(
        (
            m.bindings[i].alt,
            m.bindings[i].ctrl,
            m.bindings[i].shift,
            m.bindings[i].tilde
        ),
        (2, 2, 2, 2)
    );
    m.action(KeyboardAction::Bind(i));
    m.capture("Tab", KeyModifiers::default());
    assert_eq!(
        m.matching_functions("Tab", KeyModifiers::default()),
        vec!["Inventory", "Pickup", "DropView"]
    );
}

#[test]
fn persistence_round_trip_and_invalid_load_are_atomic() {
    let mut m = KeyboardDialogUi::default();
    m.show();
    m.action(KeyboardAction::Bind(0));
    m.capture(
        "F24",
        KeyModifiers {
            shift: true,
            ..Default::default()
        },
    );
    let saved = m.to_json().unwrap();
    let mut restored = KeyboardDialogUi::default();
    restored.load_json(&saved).unwrap();
    assert_eq!(restored.bindings, m.bindings);
    let before = restored.clone();
    let corrupt = saved.replacen("\"alt\": 0", "\"alt\": 9", 1);
    assert!(restored.load_json(&corrupt).is_err());
    assert_eq!(before, restored);
    assert!(m.action(KeyboardAction::Close));
    assert!(!m.open);
    assert!(!m.capture("A", KeyModifiers::default()));
}

#[test]
fn source_group_geometry_and_scroll_clamps() {
    let mut m = KeyboardDialogUi::default();
    m.show();
    assert_eq!(
        m.rows()[0],
        KeyboardRow::Heading {
            label: "Belt".into(),
            y: 90
        }
    );
    assert!(matches!(m.rows()[1], KeyboardRow::Binding { y: 120, .. }));
    for _ in 0..300 {
        m.action(KeyboardAction::Next);
    }
    assert_eq!(m.top_line, m.max_top_line());
    assert!(m.thumb_y() <= 344);
    m.drag_thumb(101.0);
    assert_eq!(m.top_line, 0);
    m.drag_thumb(f32::NAN);
    assert_eq!(m.top_line, 0);
    for row in m.rows() {
        let y = match row {
            KeyboardRow::Heading { y, .. } | KeyboardRow::Binding { y, .. } => y,
        };
        assert!(y <= 350);
    }
}

#[test]
fn original_target_lock_uses_current_event_even_on_key_up() {
    let mut model = KeyboardDialogUi::default();
    let bind = model
        .bindings
        .iter_mut()
        .find(|b| b.function == "TargetSpellLockOn")
        .unwrap();
    bind.key = "F1".into();
    bind.ctrl = 1;
    model.observe_lock_key_event("F1");
    assert!(model.spell_target_lock);
    model.observe_lock_key_event("F1");
    assert!(
        model.spell_target_lock,
        "KeyUp does not clear in Crystal CMain"
    );
    model.observe_lock_key_event("ControlKey");
    assert!(!model.spell_target_lock);
    model.observe_lock_key_event("F1");
    assert!(
        model.spell_target_lock,
        "the lock flag ignores modifier requirements"
    );
    model.observe_lock_key_event("F2");
    assert!(!model.spell_target_lock);
}

#[test]
fn same_frame_skill_events_keep_each_own_target_lock_flag() {
    let mut model = KeyboardDialogUi::default();
    model
        .bindings
        .iter_mut()
        .find(|b| b.function == "TargetSpellLockOn")
        .unwrap()
        .key = "F1".into();
    model.observe_lock_key_event("F1");
    model.observe_lock_key_event("F2");
    assert!(!model.spell_target_lock);
    assert!(model.lock_for_function_event("Bar1Skill1"));
    assert!(!model.lock_for_function_event("Bar1Skill2"));
}
