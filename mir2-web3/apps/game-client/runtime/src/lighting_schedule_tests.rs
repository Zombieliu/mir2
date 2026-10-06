use super::*;
use bevy::prelude::*;

#[derive(Resource, Default)]
struct CurrentFrame(u64);
fn pose(mut frame: ResMut<CurrentFrame>) {
    frame.0 += 1;
}
fn producer(frame: Res<CurrentFrame>) {
    assert!(frame.0 > 0, "Producer ran before the committed pose");
    assert!(native_ingest::push_native_lighting_render_state(
        serde_json::json!({"enabled":true,"mapFileName":format!("frame-{}",frame.0),
            "stageWidth":1024.0,"stageHeight":768.0,"timeOfDayLightSetting":4,
            "mapLights":[],"entityLights":[]})
        .to_string()
    ));
}
fn verify(frame: Res<CurrentFrame>, state: Res<RuntimeLightingRenderState>) {
    assert_eq!(
        state.snapshot.as_ref().unwrap().map_file_name.as_deref(),
        Some(format!("frame-{}", frame.0).as_str()),
        "Lighting queue was consumed before this frame's producer"
    );
}
#[test]
fn actual_native_lighting_queue_is_consumed_after_the_same_frame_pose_and_producer() {
    let _guard = native_ingest::native_queue_test_guard();
    let mut app = App::new();
    configure_native_lighting_presentation(&mut app);
    native_ingest::install_native_ingestion(&mut app);
    app.init_resource::<CurrentFrame>()
        .init_resource::<RuntimeLightingRenderState>();
    // Reverse registration order prevents accidental insertion-order success.
    app.add_systems(Update, verify.after(RuntimeLightingConsumeSet))
        .add_systems(
            Update,
            ingest_pending_lighting_render_state.in_set(RuntimeLightingConsumeSet),
        )
        .add_systems(Update, producer.in_set(NativeLightingProducerSet))
        .add_systems(Update, pose.in_set(RuntimeLightingPoseReadySet));
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(app.world().resource::<CurrentFrame>().0, 3);
}
#[test]
fn missing_optional_native_producer_keeps_existing_windows_queue_consumption() {
    let _guard = native_ingest::native_queue_test_guard();
    let mut app = App::new();
    configure_native_lighting_presentation(&mut app);
    native_ingest::install_native_ingestion(&mut app);
    app.init_resource::<RuntimeLightingRenderState>()
        .add_systems(
            Update,
            ingest_pending_lighting_render_state.in_set(RuntimeLightingConsumeSet),
        );
    assert!(native_ingest::push_native_lighting_render_state(
        serde_json::json!({
        "enabled":true,"stageWidth":1024.0,"stageHeight":768.0,"timeOfDayLightSetting":4,
        "mapFileName":"windows","mapLights":[],"entityLights":[]})
        .to_string()
    ));
    app.update();
    assert_eq!(
        app.world()
            .resource::<RuntimeLightingRenderState>()
            .snapshot
            .as_ref()
            .unwrap()
            .map_file_name
            .as_deref(),
        Some("windows")
    );
}
