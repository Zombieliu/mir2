use super::*;
fn fixture() -> (SpellsUiContext, SpellsUiState) {
    let c = SpellsUiContext {
        identity: SpellsIdentity {
            request_run: 7,
            connection_generation: 1,
            session_generation: 2,
            owner_revision: 0,
            hud_generation: 3,
            player_object_id: 4,
        },
        revision: 1,
        model_revision: 1,
        presentation_revision: 1,
        active: true,
        ready: true,
        input_enabled: true,
        now_ms: 10,
        ..Default::default()
    };
    let model:SkillModel=serde_json::from_value(serde_json::json!({"authority":{"sessionEpoch":2,"snapshotSerial":1,"playerObjectId":4},"skills":[{"id":0,"name":"火球","spell":"FireBall","icon":0,"hotkey":1},{"id":1,"name":"治疗","spell":"Healing","icon":1,"hotkey":16}]})).unwrap();
    let mut state = SpellsUiState::default();
    state.ingest(&c, &model, vec![]);
    (c, state)
}
fn edge(
    c: &SpellsUiContext,
    s: &SpellsUiState,
    sequence: u64,
    pointer_id: u64,
    phase: &str,
    x: f32,
    y: f32,
) -> SpellsPointerEdge {
    SpellsPointerEdge {
        identity: c.identity,
        sequence,
        model_revision: c.model_revision,
        presentation_revision: c.presentation_revision,
        render_revision: s.render_revision,
        pointer_id,
        phase: phase.into(),
        x,
        y,
        button: 0,
    }
}
#[test]
fn m10_portable_pointer_crossing_second_pointer_and_revision_retire_save() {
    let (c, mut s) = fixture();
    let mut q = SpellsIntentQueue::default();
    let e = edge(&c, &s, 1, 1, "down", 814., 100.);
    assert!(s.process(&c, e, &mut q));
    let e = edge(&c, &s, 2, 1, "move", 400., 500.);
    assert!(!s.process(&c, e, &mut q));
    let e = edge(&c, &s, 3, 1, "up", 814., 100.);
    s.process(&c, e, &mut q);
    assert!(!s.draft.open);
    assert!(q.0.is_empty());
    let e = edge(&c, &s, 4, 1, "down", 814., 100.);
    s.process(&c, e, &mut q);
    let e = edge(&c, &s, 5, 2, "down", 814., 100.);
    assert!(!s.process(&c, e, &mut q));
    let e = edge(&c, &s, 6, 1, "up", 814., 100.);
    s.process(&c, e, &mut q);
    assert!(!s.draft.open);
    let mut e = edge(&c, &s, 7, 1, "down", 814., 100.);
    e.model_revision += 1;
    assert!(!s.process(&c, e, &mut q));
}
#[test]
fn m10_portable_real_actions_select_without_cast_and_dispatch_tristate() {
    for outcome in [
        SpellsTransportOutcome::ConfirmedSend,
        SpellsTransportOutcome::DefinitelyUnsent,
        SpellsTransportOutcome::OutcomeUnknown,
    ] {
        let (c, mut s) = fixture();
        let mut q = SpellsIntentQueue::default();
        let e = edge(&c, &s, 1, 1, "down", 814., 100.);
        s.process(&c, e, &mut q);
        let e = edge(&c, &s, 2, 1, "up", 814., 100.);
        s.process(&c, e, &mut q);
        assert!(s.draft.open);
        assert!(q.0.is_empty());
        s.draft.choose(16);
        s.update_render(c.now_ms);
        let rect = s
            .targets(c.now_ms)
            .into_iter()
            .find(|(a, _)| *a == SkillPageAction::Save)
            .unwrap()
            .1;
        let e = edge(&c, &s, 3, 1, "down", rect.left + 1., rect.top + 1.);
        s.process(&c, e, &mut q);
        let e = edge(&c, &s, 4, 1, "up", rect.left + 1., rect.top + 1.);
        s.process(&c, e, &mut q);
        assert_eq!(q.0.len(), 1);
        let i = q.0.remove(0);
        assert_eq!(i.request.request_id, 7 * (1 << 22) + 1);
        assert_eq!(i.request.spell, "FireBall");
        assert_eq!(i.render_revision, s.render_revision);
        s.dispatched(&c, &i, outcome);
        match outcome {
            SpellsTransportOutcome::ConfirmedSend => {
                assert!(s.authority.is_pending());
                assert_eq!(s.view.binding_for(0).hotkey, Some(16));
                assert_eq!(s.view.binding_for(1).hotkey, Some(0));
            }
            SpellsTransportOutcome::DefinitelyUnsent => {
                assert!(!s.authority.is_pending());
                assert!(s.draft.open);
                assert!(!s.draft.pending);
                assert_eq!(s.view.binding_for(0).hotkey, Some(1));
            }
            SpellsTransportOutcome::OutcomeUnknown => {
                assert!(s.authority.is_pending());
                assert_eq!(s.view.binding_for(0).hotkey, Some(1));
                assert_eq!(s.view.binding_for(1).hotkey, Some(16));
            }
        }
    }
}
#[test]
fn m10_portable_clear_same_run_and_owner_change_never_recycle_requests() {
    let (mut c, mut s) = fixture();
    let model = s.view.clone();
    let mut q = SpellsIntentQueue::default();
    let mut seq = 1;
    fn save(
        c: &SpellsUiContext,
        s: &mut SpellsUiState,
        q: &mut SpellsIntentQueue,
        seq: &mut u64,
    ) -> u64 {
        s.draft.show(0, &s.view);
        s.draft.choose(16);
        s.update_render(c.now_ms);
        let r = s
            .targets(c.now_ms)
            .into_iter()
            .find(|(a, _)| *a == SkillPageAction::Save)
            .unwrap()
            .1;
        for phase in ["down", "up"] {
            let e = edge(c, s, *seq, 1, phase, r.left + 1., r.top + 1.);
            *seq += 1;
            s.process(c, e, q);
        }
        let i = q.0.remove(0);
        let id = i.request.request_id;
        s.dispatched(c, &i, SpellsTransportOutcome::DefinitelyUnsent);
        id
    }
    let first = save(&c, &mut s, &mut q, &mut seq);
    // The actual rejected snapshot / clear_sink method preserves its namespace.
    s.clear_snapshot();
    c.active = false;
    s.ingest(&c, &model, vec![]);
    c.active = true;
    s.ingest(&c, &model, vec![]);
    let second = save(&c, &mut s, &mut q, &mut seq);
    assert!(second > first);
    c.identity.owner_revision += 1;
    s.ingest(&c, &model, vec![]);
    let third = save(&c, &mut s, &mut q, &mut seq);
    assert!(third > second);
    c.identity.request_run += 1;
    s.ingest(&c, &model, vec![]);
    seq = 1;
    let fresh = save(&c, &mut s, &mut q, &mut seq);
    assert!(fresh > third);
    c.identity.request_run -= 1;
    s.ingest(&c, &model, vec![]);
    assert!(
        s.namespace.is_none(),
        "retired run cannot create a new counter"
    );
}
#[test]
fn m10_portable_full_queue_burns_request_before_real_save_retry() {
    let (c, mut s) = fixture();
    let mut q = SpellsIntentQueue::default();
    for _ in 0..64 {
        q.0.push(SpellsIntent {
            identity: c.identity,
            model_revision: c.model_revision,
            presentation_revision: c.presentation_revision,
            render_revision: s.render_revision,
            skill_id: 0,
            request: SkillKeyRequest {
                request_id: 1,
                spell: "FireBall".into(),
                key: 16,
                old_key: 1,
            },
        });
    }
    s.draft.show(0, &s.view);
    s.draft.choose(16);
    s.update_render(c.now_ms);
    let r = s
        .targets(c.now_ms)
        .into_iter()
        .find(|(a, _)| *a == SkillPageAction::Save)
        .unwrap()
        .1;
    for (seq, phase) in [(1, "down"), (2, "up")] {
        let e = edge(&c, &s, seq, 1, phase, r.left + 1., r.top + 1.);
        s.process(&c, e, &mut q);
    }
    assert_eq!(q.0.len(), 64);
    assert!(!s.draft.pending);
    let burned = s.draft.request_id;
    q.0.clear();
    for (seq, phase) in [(3, "down"), (4, "up")] {
        let e = edge(&c, &s, seq, 1, phase, r.left + 1., r.top + 1.);
        s.process(&c, e, &mut q);
    }
    assert_eq!(q.0.len(), 1);
    assert_eq!(q.0[0].request.request_id, burned + 1);
}
#[test]
#[ignore = "Root supplies an actual executed JS Host pointer setter capture"]
fn m10_actual_js_spells_pointer_capture_matches_strict_rust_edge() {
    let path = std::env::var("MIR2_M10_POINTER_FIXTURE").expect("explicit Root pointer capture");
    let e: SpellsPointerEdge =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert!(e.identity.valid());
    assert_eq!(e.phase, "down");
    assert!(e.sequence > 0);
    assert!(e.x.is_finite() && e.y.is_finite());
}
