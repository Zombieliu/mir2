use super::*;
fn model() -> SkillModel {
    serde_json::from_value(serde_json::json!({"authority":{"sessionEpoch":3,"playerObjectId":7,"snapshotSerial":1},"skills":[{"id":0,"name":"Localized fire","spell":"FireBall","hotkey":1,"castSequence":2,"delayMs":3400},{"id":1,"name":"Healing","spell":"Healing","hotkey":16}]})).unwrap()
}
#[test]
fn m10_request_namespace_burns_and_stays_js_safe_at_both_boundaries() {
    let mut old = PortableRequestNamespace::new(1).unwrap();
    let mut fresh = PortableRequestNamespace::new(2).unwrap();
    let a = old.allocate().unwrap();
    let b = fresh.allocate().unwrap();
    assert_ne!(a, b);
    assert!(!fresh.owns(a));
    assert!(fresh.owns(b));
    assert_ne!(old.allocate().unwrap(), a);
    let mut last = PortableRequestNamespace::new(MAX_REQUEST_RUN).unwrap();
    last.counter = REQUEST_BLOCK - 2;
    assert_eq!(last.allocate(), Some(MAX_SAFE_ID));
    assert_eq!(last.allocate(), None);
    assert_eq!(last.allocate(), None);
    assert!(PortableRequestNamespace::new(0).is_none());
    assert!(PortableRequestNamespace::new(MAX_REQUEST_RUN + 1).is_none());
}
#[test]
fn m10_pending_requires_exact_receipt_and_rejects_same_id_different_spell() {
    let base = model();
    let mut draft = SkillAssignUi::default();
    draft.show(0, &base);
    draft.choose(16);
    draft.request(REQUEST_BLOCK + 1, &base).unwrap();
    let mut authority = SkillAuthorityUi::default();
    authority.begin(&base, draft.clone());
    let mut stale = base.clone();
    authority.reconcile(&mut stale);
    assert_eq!(stale.binding_for(0).hotkey, Some(16));
    assert_eq!(stale.binding_for(1).hotkey, Some(0));
    stale = base.clone();
    stale.skill_key_ack = Some(crate::skill_model::SkillKeyAck {
        request_id: REQUEST_BLOCK + 2,
        spell: "FireBall".into(),
        key: 16,
        old_key: 1,
        accepted: false,
    });
    assert!(authority.reconcile(&mut stale).is_none());
    assert!(authority.is_pending());
    stale = base.clone();
    stale.skills.remove(0);
    stale.skills[0].id = 0;
    stale.bindings.remove(0);
    stale.bindings[0].skill_id = 0;
    assert!(authority.reconcile(&mut stale).is_none());
    assert!(!authority.is_pending());
    assert_eq!(stale.binding_for(0).hotkey, Some(16));
    assert!(!draft.valid(&stale));
    let mut authority = SkillAuthorityUi::default();
    authority.begin(&base, draft);
    let mut rejected = base.clone();
    rejected.skill_key_ack = Some(crate::skill_model::SkillKeyAck {
        request_id: REQUEST_BLOCK + 1,
        spell: "FireBall".into(),
        key: 16,
        old_key: 1,
        accepted: false,
    });
    let restored = authority.reconcile(&mut rejected).unwrap();
    assert!(restored.valid(&rejected));
    assert!(!restored.pending);
    assert_eq!(rejected.binding_for(0).hotkey, Some(1));
}
#[test]
fn m10_owner_duplicate_and_missing_spell_cannot_project_or_restore() {
    let base = model();
    let mut draft = SkillAssignUi::default();
    draft.show(0, &base);
    let mut changed = base.clone();
    changed.authority.session_epoch += 1;
    assert!(!draft.valid(&changed));
    let mut duplicate = base.clone();
    duplicate.skills.push(base.skills[0].clone());
    duplicate.bindings.push(base.bindings[0].clone());
    assert!(!draft.valid(&duplicate));
    assert!(!apply_source_key(&mut duplicate, 0, "FireBall", 16));
    let mut missing = base.clone();
    missing.bindings[0].spell = None;
    assert!(!draft.valid(&missing));
}
#[test]
fn m10_unknown_send_waits_without_projection_until_exact_receipt() {
    let base = model();
    let mut draft = SkillAssignUi::default();
    draft.show(0, &base);
    draft.choose(16);
    draft.request(REQUEST_BLOCK + 17, &base).unwrap();
    draft.queued(true);
    let mut unknown = SkillAuthorityUi::default();
    unknown.begin_unknown(&base, draft.clone());
    for serial in [2, 3, 4] {
        let mut snapshot = base.clone();
        snapshot.authority.snapshot_serial = serial;
        unknown.reconcile(&mut snapshot);
        assert!(unknown.is_pending());
        assert_eq!(snapshot.binding_for(0).hotkey, Some(1));
        assert_eq!(snapshot.binding_for(1).hotkey, Some(16));
    }
    let mut rejected = base.clone();
    rejected.skill_key_ack = Some(crate::skill_model::SkillKeyAck {
        request_id: REQUEST_BLOCK + 17,
        spell: "FireBall".into(),
        key: 16,
        old_key: 1,
        accepted: false,
    });
    let restored = unknown.reconcile(&mut rejected).unwrap();
    assert!(restored.valid(&rejected));
    assert!(!restored.pending);
    assert!(!unknown.is_pending());
    assert_eq!(rejected.binding_for(0).hotkey, Some(1));
    let mut accepted = base.clone();
    accepted.bindings[0].hotkey = Some(16);
    accepted.bindings[1].hotkey = Some(0);
    accepted.skill_key_ack = Some(crate::skill_model::SkillKeyAck {
        accepted: true,
        ..rejected.skill_key_ack.clone().unwrap()
    });
    unknown.begin_unknown(&base, draft.clone());
    assert!(unknown.reconcile(&mut accepted).is_none());
    assert!(!unknown.is_pending());
    assert_eq!(accepted.binding_for(0).hotkey, Some(16));
    let mut confirmed = SkillAuthorityUi::default();
    confirmed.begin(&base, draft);
    let mut projection = base.clone();
    confirmed.reconcile(&mut projection);
    assert_eq!(projection.binding_for(0).hotkey, Some(16));
    assert_eq!(projection.binding_for(1).hotkey, Some(0));
}
#[test]
fn m10_definitely_unsent_burns_id_and_invalid_selection_retires_draft() {
    let base = model();
    let mut draft = SkillAssignUi::default();
    let mut namespace = PortableRequestNamespace::new(3).unwrap();
    draft.show(0, &base);
    let failed = namespace.allocate().unwrap();
    draft.request(failed, &base).unwrap();
    draft.queued(false);
    assert!(!draft.pending);
    assert!(draft.valid(&base));
    assert!(namespace.allocate().unwrap() > failed);
    assert_eq!(base.binding_for(0).hotkey, Some(1));
    draft.show(999, &base);
    assert!(!draft.open);
    assert_eq!(draft.request_id, 0);
    assert!(draft.spell.is_empty());
}
#[test]
fn m10_shared_clock_does_not_restart_on_snapshot_or_delay_change() {
    let mut s = model();
    let mut clock = SkillCooldownClock::default();
    clock.observe(&s, 100);
    assert_eq!(clock.frame(0, &s, 100, 1290, 34), Some(1290));
    clock.observe(&s, 1100);
    assert_eq!(clock.remaining_ms(0, &s, 1100), 2400);
    s.bindings[0].delay_ms = Some(4400);
    assert_eq!(clock.remaining_ms(0, &s, 1100), 3400);
    s.bindings[0].spell = Some("Healing".into());
    clock.observe(&s, 1200);
    assert_eq!(clock.remaining_ms(0, &s, 1200), 0);
}
#[test]
fn m10_raw_catalog_is_absence_only_and_explicit_zero_null_are_retained() {
    let rows = vec![
        serde_json::json!({"spell":"FireBall","magicName":"火球","level":0,"icon":0,"experience":0,"need1":0,"need2":null,"delayMs":0}),
    ];
    let s = normalize_raw_skills(&rows).unwrap();
    let b = s.binding_for(0);
    assert_eq!(s.skills[0].name, "火球");
    assert_eq!(b.icon, Some(0));
    assert_eq!(b.need1, Some(0));
    assert_eq!(b.need2, None);
    assert!(b.need3.is_some());
}
#[test]
fn m10_portable_clock_uses_actual_cast_observation_and_never_delay_as_cast() {
    let mut s = model();
    let mut clock = SkillCooldownClock::default();
    clock.observe_with_starts(&s, 1100, |_, _, _| Some(100));
    assert_eq!(clock.remaining_ms(0, &s, 1100), 2400);
    clock.observe_with_starts(&s, 1200, |_, _, _| Some(1150));
    assert_eq!(clock.remaining_ms(0, &s, 1200), 2300);
    s.bindings[0].delay_ms = Some(4400);
    clock.observe_with_starts(&s, 1300, |_, _, _| Some(1250));
    assert_eq!(clock.remaining_ms(0, &s, 1300), 3200);
    s.bindings[0].cast_sequence += 1;
    clock.observe_with_starts(&s, 1400, |_, _, _| Some(1350));
    assert_eq!(clock.remaining_ms(0, &s, 1400), 4350);
}
#[test]
fn m10_raw_null_name_delay_and_missing_delay_remain_unknown() {
    let rows = vec![
        serde_json::json!({"spell":"FireBall","magicName":null,"icon":null,"delayMs":null}),
        serde_json::json!({"spell":"Healing","name":null}),
    ];
    let s = normalize_raw_skills(&rows).unwrap();
    assert!(s.skills.iter().all(|s| s.name.is_empty()));
    assert_eq!(s.binding_for(0).icon, None);
    assert!(s.bindings.iter().all(|b| b.delay_ms.is_none()));
    let s = normalize_raw_skills(&[serde_json::json!({"spell":"FireBall","delayMs":0})]).unwrap();
    assert_eq!(s.binding_for(0).delay_ms, Some(0));
    assert!(!s.skills[0].name.is_empty());
}
