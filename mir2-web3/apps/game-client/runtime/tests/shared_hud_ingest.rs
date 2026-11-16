#![cfg(feature = "web-quest-ui")]
use mir2_bevy_runtime::hud_ui_host::HudUiSnapshot;
use mir2_client_bevy::crystal_ui::panel_navigation::*;
use mir2_client_bevy::read_model::*;
fn snapshot(generation: u64, revision: u64) -> HudUiSnapshot {
    serde_json::from_value(serde_json::json!({"generation":generation,"revision":revision,"inGame":true,"hostVisible":true,
        "logicalWidth":1024,"logicalHeight":768,"stageCssScale":1,"touch":true,
        "player":{"hp":3,"maxHp":10,"mp":2,"maxMp":8,"name":"Authority","gold":22,"level":12,
            "crystalStats":[{"stat":0,"value":8}],"weights":{"bag":4294967295_u32,"wear":0,"hand":2}},
        "navigation":{"characterOpen":true,"characterPage":"stats2","bagOpen":false,"questOpen":true}})).unwrap()
}
#[test]
fn full_hud_ingest_is_atomic_and_independent_from_quest_revision() {
    let mut ingress = UiReadModelIngress::default();
    let mut model = UiReadModel::default();
    let mut nav = PanelNavigation::default();
    assert!(snapshot(1, 3).apply(&mut ingress, &mut model, &mut nav));
    assert!(nav.character_open);
    let prior_nav = nav;
    assert!(!ingress.apply_legacy_quest(&mut model, 1, 999, PlayerStats::default()));
    assert_eq!(nav, prior_nav);
    assert_eq!(model.player.gold, 22);
    assert_eq!(model.player.weights.unwrap().bag, u32::MAX);
    assert!(ingress.apply_legacy_quest(&mut model, 2, 999, PlayerStats::default()));
    assert!(!ingress.has_complete_generation(1));
    assert!(snapshot(2, 1).apply(&mut ingress, &mut model, &mut nav));
    assert_eq!(model.player.name.as_deref(), Some("Authority"));
    assert!(!snapshot(1, 9999).apply(&mut ingress, &mut model, &mut nav));
    let mut full = snapshot(2, 2);
    full.player = None;
    assert!(full.apply(&mut ingress, &mut model, &mut nav));
    assert_eq!(model.player, PlayerStats::default());
    assert_eq!(nav, PanelNavigation::default());
}
#[test]
fn logout_full_snapshot_clears_but_surface_withdrawal_preserves_authority() {
    let mut ingress = UiReadModelIngress::default();
    let mut model = UiReadModel::default();
    let mut nav = PanelNavigation::default();
    assert!(snapshot(3, 1).apply(&mut ingress, &mut model, &mut nav));
    let mut hidden = snapshot(3, 2);
    hidden.host_visible = false;
    assert!(hidden.apply(&mut ingress, &mut model, &mut nav));
    assert_eq!(model.player.gold, 22);
    let mut logout = snapshot(3, 3);
    logout.in_game = false;
    assert!(logout.apply(&mut ingress, &mut model, &mut nav));
    assert_eq!(model.player, PlayerStats::default());
    assert_eq!(nav, PanelNavigation::default());
}
#[test]
fn name_is_optional_and_explicit_fallback_navigation_recovery_has_its_own_clock() {
    let mut ingress = UiReadModelIngress::default();
    let mut model = UiReadModel::default();
    let mut nav = PanelNavigation::default();
    let mut full = snapshot(1, 1);
    full.player.as_mut().unwrap().name = None;
    assert!(full.apply(&mut ingress, &mut model, &mut nav));
    assert!(nav.character_open);
    nav = reduce(nav, PanelAction::CloseCharacter);
    full.revision = 2;
    assert!(full.apply(&mut ingress, &mut model, &mut nav));
    assert!(
        !nav.character_open,
        "React projection cannot clobber local navigation"
    );
    full.navigation_revision = 1;
    full.revision = 3;
    assert!(full.apply(&mut ingress, &mut model, &mut nav));
    assert!(
        nav.character_open,
        "explicit fallback navigation resumes once"
    );
    full.player = None;
    full.revision = 4;
    assert!(full.apply(&mut ingress, &mut model, &mut nav));
    assert_eq!(nav, PanelNavigation::default());
}
#[test]
fn ready_and_root_visibility_yield_and_recover_together_without_losing_layout() {
    use bevy::prelude::{Node, Val, Visibility, World};
    use mir2_bevy_runtime::hud_ui_host::{complete_frame_ready, frame_visibility};
    use mir2_client_bevy::portable_hp_orb_ui::{HpOrbObservation, HpOrbSlot, MpOrbObservation};
    let model = UiReadModel {
        player: snapshot(1, 1).player.unwrap(),
    };
    let mut hp = HpOrbObservation {
        ready: true,
        generation: 1,
        hp: 3,
        max_hp: 10,
        hp_only: false,
        slot: Some(HpOrbSlot {
            left: 0.,
            top: 646.,
        }),
        ..Default::default()
    };
    let mp = MpOrbObservation {
        ready: true,
        generation: 1,
        mp: 2,
        max_mp: 8,
        slot: hp.slot,
        ..Default::default()
    };
    let mut world = World::new();
    let root = world
        .spawn((
            Node {
                width: Val::Px(1024.),
                ..Default::default()
            },
            frame_visibility(false),
        ))
        .id();
    for (active, generation, orb_ready, expected) in [
        (true, 1, false, false),
        (true, 1, true, true),
        (true, 2, true, false),
        (false, 1, true, false),
        (true, 1, true, true),
    ] {
        hp.ready = orb_ready;
        let ready = complete_frame_ready(active, generation, &model, &hp, &mp, true);
        world.entity_mut(root).insert(frame_visibility(ready));
        assert_eq!(ready, expected);
        assert_eq!(
            *world.get::<Visibility>(root).unwrap(),
            if expected {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            }
        );
        assert_eq!(
            world.get::<Node>(root).unwrap().width,
            Val::Px(1024.),
            "withdrawal keeps measurement geometry"
        );
    }
    hp.hp = 4;
    assert!(
        !complete_frame_ready(true, 1, &model, &hp, &mp, true),
        "same-generation stale values also withdraw"
    );
}
#[test]
fn new_hud_snapshot_rejects_unknown_nested_authority_fields() {
    let mut json = serde_json::to_value(snapshot_json()).unwrap();
    json["player"]["unknown"] = true.into();
    assert!(serde_json::from_value::<HudUiSnapshot>(json).is_err());
}
fn snapshot_json() -> serde_json::Value {
    serde_json::json!({"generation":1,"revision":1,"inGame":true,"hostVisible":true,"logicalWidth":1024,"logicalHeight":768,"stageCssScale":1,"touch":true,
        "navigation":{"characterOpen":false,"characterPage":"character","bagOpen":false,"questOpen":false},"player":{"hp":1,"maxHp":2}})
}
