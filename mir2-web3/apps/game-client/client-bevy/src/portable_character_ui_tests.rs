use super::*;
fn fixture() -> (
    CharacterUiContext,
    CharacterUiReadModel,
    CharacterUiState,
    CharacterIntentQueue,
) {
    let c = CharacterUiContext {
        identity: CharacterIdentity {
            run_generation: 1,
            connection_generation: 1,
            session_generation: 2,
            hud_generation: 3,
            ..default()
        },
        revision: 1,
        model_revision: 1,
        presentation_revision: 1,
        active: true,
        ready: true,
        input_enabled: true,
        ..default()
    };
    let mut m = CharacterUiReadModel::default();
    m.0.inventory.items = vec![crate::inventory::ItemModel {
        container: 2,
        slot: 0,
        unique_id: Some(0),
        quantity: 1,
        ..default()
    }];
    (
        c,
        m,
        CharacterUiState::default(),
        CharacterIntentQueue::default(),
    )
}
fn edge(c: &CharacterUiContext, n: u64, phase: &str, x: f32, y: f32) -> CharacterPointerEdge {
    CharacterPointerEdge {
        identity: c.identity,
        sequence: n,
        model_revision: c.model_revision,
        presentation_revision: c.presentation_revision,
        pointer_id: 7,
        phase: phase.into(),
        x,
        y,
        button: 0,
    }
}
#[test]
fn m9_inspect_then_remove_emits_current_uid_zero_and_keeps_equipment_unchanged() {
    let (c, m, mut s, mut q) = fixture();
    assert!(s.process(&c, &m, edge(&c, 1, "down", 900., 110.), &mut q));
    s.process(&c, &m, edge(&c, 2, "up", 900., 110.), &mut q);
    assert_eq!(
        s.selected,
        Some(CharacterSource {
            container: 2,
            slot: 0,
            unique_id: 0
        })
    );
    s.process(&c, &m, edge(&c, 3, "down", 490., 350.), &mut q);
    s.process(&c, &m, edge(&c, 4, "up", 490., 350.), &mut q);
    assert_eq!(q.intents.len(), 1);
    assert_eq!(q.intents[0].source.unique_id, 0);
    assert_eq!(m.0.inventory.items[0].container, 2);
}
#[test]
fn m9_crossing_and_second_pointer_cancel_instead_of_selecting_or_mutating() {
    let (c, m, mut s, mut q) = fixture();
    s.process(&c, &m, edge(&c, 1, "down", 900., 110.), &mut q);
    s.process(&c, &m, edge(&c, 2, "move", 700., 110.), &mut q);
    s.process(&c, &m, edge(&c, 3, "up", 900., 110.), &mut q);
    assert!(s.selected.is_none());
    s.process(&c, &m, edge(&c, 4, "down", 900., 110.), &mut q);
    let mut second = edge(&c, 5, "up", 900., 110.);
    second.pointer_id = 8;
    assert!(!s.process(&c, &m, second, &mut q));
    assert!(s.selected.is_none());
    assert!(q.intents.is_empty());
}
#[test]
fn m9_pending_and_replaced_source_fail_closed() {
    let (c, mut m, mut s, mut q) = fixture();
    s.process(&c, &m, edge(&c, 1, "down", 900., 110.), &mut q);
    m.0.inventory.items[0].unique_id = Some(1);
    s.process(&c, &m, edge(&c, 2, "up", 900., 110.), &mut q);
    assert!(s.selected.is_none());
    m.0.inventory.items[0].unique_id = Some(0);
    s.process(&c, &m, edge(&c, 3, "down", 900., 110.), &mut q);
    s.process(&c, &m, edge(&c, 4, "up", 900., 110.), &mut q);
    m.0.blocked_unique_ids.insert(0);
    s.process(&c, &m, edge(&c, 5, "down", 490., 350.), &mut q);
    s.process(&c, &m, edge(&c, 6, "up", 490., 350.), &mut q);
    assert!(q.intents.is_empty());
}
#[test]
fn m9_stale_cancel_cannot_cancel_new_generation_or_poison_sequence() {
    let (mut c, m, mut s, mut q) = fixture();
    let old = c.clone();
    c.identity.run_generation = 2;
    s.process(&c, &m, edge(&c, 1, "down", 900., 110.), &mut q);
    assert!(!s.process(&c, &m, edge(&old, 999, "cancel", 0., 0.), &mut q));
    s.process(&c, &m, edge(&c, 2, "up", 900., 110.), &mut q);
    assert!(s.selected.is_some());
}
#[test]
fn m9_production_pointer_json_is_strict_and_uid_zero_remains_a_source() {
    let (c, _, _, _) = fixture();
    let v = serde_json::json!({"runGeneration":c.identity.run_generation,"connectionGeneration":1,"sessionGeneration":2,"ownerRevision":0,"ledgerRunGeneration":0,"hudGeneration":3,
        "sequence":1,"modelRevision":1,"presentationRevision":1,"pointerId":7,"phase":"down","x":900,"y":110,"button":0});
    let edge: CharacterPointerEdge = serde_json::from_value(v.clone()).unwrap();
    assert_eq!(edge.identity, c.identity);
    let mut extra = v;
    extra["sourceUniqueIdGuess"] = 0.into();
    assert!(serde_json::from_value::<CharacterPointerEdge>(extra).is_err());
    let source: CharacterSource =
        serde_json::from_str(r#"{"container":2,"slot":0,"uniqueId":0}"#).unwrap();
    assert_eq!(source.unique_id, 0);
}
#[test]
#[ignore = "requires the actual Node Host-to-Router runtime setter capture in MIR2_M9_POINTER_FIXTURE"]
fn m9_actual_web_pointer_setter_capture_deserializes_with_production_dto() {
    let path =
        std::env::var("MIR2_M9_POINTER_FIXTURE").expect("provide Node runtime setter capture");
    let json = std::fs::read_to_string(path).expect("read captured pointer JSON");
    let edge: CharacterPointerEdge =
        serde_json::from_str(&json).expect("production strict pointer DTO");
    assert_eq!(
        (
            edge.pointer_id,
            edge.phase.as_str(),
            edge.x,
            edge.y,
            edge.button
        ),
        (7, "down", 900., 110., 0)
    );
    assert!(edge.identity.run_generation > 0);
}
#[test]
fn m9_required_assets_include_every_actual_character_chrome_and_class_image() {
    let (_, mut m, _, _) = fixture();
    m.0.player.class_name = Some("warrior".into());
    m.0.player.gender = Some("male".into());
    m.0.inventory.items[0].icon = 71;
    m.0.inventory.items[0].tooltip_source = Some(crate::inventory::CrystalItemTooltipSourceModel {
        info: crate::inventory::CrystalItemInfoModel {
            image: 0,
            ..default()
        },
        ..default()
    });
    let paths = required_assets(&m);
    assert!(paths.contains(&"original-ui/Items/0.png".to_owned()));
    assert!(!paths.contains(&"original-ui/Items/71.png".to_owned()));
    for path in [
        "original-ui/Title/504.png",
        "original-ui/Title/500.png",
        "original-ui/Prguse2/360.png",
        "original-ui/Prguse/340.png",
    ] {
        assert!(paths.contains(&path.to_owned()), "missing {path}");
    }
    let class = crate::crystal_ui::character_stats::class_image_index(Some("warrior")).unwrap();
    assert!(paths.contains(&format!("original-ui/Prguse/{class}.png")));
    assert!(
        !paths.iter().any(|p| p == "original-ui/Title/506.png"),
        "Stats-only art does not qualify Character"
    );
}
