//! A live zero-percent bar keeps its border and the original display deadline.
use super::*;
use serde_json::json;

#[test]
fn health_zero_live_bar_retains_exact_deadline_without_snapshot_renewal() {
    let mut state = MonsterHealthState::default();
    let mut actor = json!({"objectId":77,"kind":"monster","dead":false,"_healthGeneration":1,"_healthRevision":1,"_healthPercent":0,"_healthExpireSeconds":5});
    assert_eq!(state.visible(&actor, 100), Some(("77".into(), 0)));
    assert_eq!(state.visible(&actor, 5099), Some(("77".into(), 0)));
    assert!(state.visible(&actor, 5100).is_none());
    assert!(
        state.visible(&actor, 5200).is_none(),
        "a repeated snapshot cannot renew Expire"
    );
    actor["_healthRevision"] = json!(2);
    assert_eq!(state.visible(&actor, 5300), Some(("77".into(), 0)));
    actor["dead"] = json!(true);
    assert!(state.visible(&actor, 5301).is_none());
}

#[test]
fn health_zero_display_generation_reuse_has_no_old_reveal_window() {
    let mut state = MonsterHealthState::default();
    let mut actor = json!({"objectId":77,"kind":"monster","dead":false,"_healthGeneration":1,"_healthRevision":1,"_healthPercent":0,"_healthExpireSeconds":5});
    assert!(state.visible(&actor, 100).is_some());
    actor["_healthGeneration"] = json!(2);
    actor["_healthExpireSeconds"] = json!(0);
    assert!(state.visible(&actor, 200).is_none());
    actor["_healthRevision"] = json!(2);
    actor["_healthExpireSeconds"] = json!(5);
    assert_eq!(state.visible(&actor, 300), Some(("77".into(), 0)));
}

#[test]
fn health_zero_actual_ui_keeps_empty_fill_and_border_until_exact_expiry() {
    let mut app = App::new();
    app.add_plugins((
        bevy::app::TaskPoolPlugin::default(),
        bevy::asset::AssetPlugin::default(),
    ));
    app.init_asset::<Image>();
    let images = [
        app.world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::default()),
        app.world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::default()),
    ];
    app.insert_resource(MonsterHealthState {
        images: Some(images),
        ..default()
    });
    app.insert_resource(Time::<()>::default());
    app.insert_resource(NativeShellModel {
        screen: NativeShellScreen::InGame,
        ..default()
    });
    app.init_resource::<NativeEntityPresentation>();
    app.init_resource::<PresentationPoseBuffer>();
    let payload = json!({"sceneView":{"center":{"x":10,"y":20}},"entities":[{"objectId":77,"kind":"monster","x":11,"y":20,"hp":1,"maxHp":200,"dead":false,"_healthRevision":1,"_healthPercent":0,"_healthExpireSeconds":5}]});
    let mut overlays = NativeEntityOverlays::default();
    overlays.replace_payload(payload);
    app.insert_resource(overlays);
    app.add_systems(Update, sync_native_monster_health);
    app.update();
    let root = app
        .world_mut()
        .query_filtered::<Entity, With<HealthRoot>>()
        .single(app.world())
        .unwrap();
    let fill = app
        .world_mut()
        .query_filtered::<Entity, With<HealthFill>>()
        .single(app.world())
        .unwrap();
    assert_eq!(app.world().get::<Node>(root).unwrap().width, Val::Px(32.0));
    assert_eq!(app.world().get::<Node>(fill).unwrap().width, Val::Px(0.0));
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::from_millis(4999));
    app.update();
    assert!(app.world().get::<HealthRoot>(root).is_some());
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::from_millis(1));
    app.update();
    assert!(app.world().get_entity(root).is_err());
    assert!(app.world().get_entity(fill).is_err());
    app.update();
    assert_eq!(
        app.world_mut()
            .query_filtered::<Entity, With<HealthRoot>>()
            .iter(app.world())
            .count(),
        0
    );
}
