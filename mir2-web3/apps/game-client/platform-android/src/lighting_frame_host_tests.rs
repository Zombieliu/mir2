use super::*;
use crate::lighting_render::AndroidLightingAssetFrame;
use mir2_client_bevy::native_lighting_sources::{
    NativeLightAssets, NativeLightingMotion, NativeMapLightCell,
};
fn raw(map: &str, name: &str) -> String {
    json!({"playerObjectId":42,"mapFileName":map,"lightSetting":4,
        "entities":[{"kind":"selfPlayer","objectId":42,"name":name,"x":10,"y":20}]})
    .to_string()
}
fn host() -> HostState {
    let mut host = HostState {
        phase: "IN_GAME".into(),
        ..Default::default()
    };
    let world = raw("0", "Fixture");
    host.player.snapshot(&world).unwrap();
    host.bind_lighting_snapshot(&world).unwrap();
    assert!(host.lighting.render.expect_render(10, (10, 20)));
    assert!(
        host.lighting
            .render
            .admit_sources(AndroidLightingAssetFrame {
                request_id: 10,
                map_file_name: "0".into(),
                center: (10, 20),
                cells: vec![NativeMapLightCell {
                    key: "11:20:1".into(),
                    x: 11,
                    y: 20,
                    light: 1,
                    offset_x: 0,
                    offset_y: 0
                }],
                assets: NativeLightAssets::complete_fixture(),
            })
    );
    host
}
fn frame(host: &mut HostState) -> Value {
    let environment = host.lighting.render_environment().clone();
    host.lighting
        .render
        .produce(
            &environment,
            true,
            Some((10, 20)),
            |_| true,
            &NativeLightingMotion::default(),
            &[],
        )
        .unwrap();
    let mut output = None;
    assert!(host.lighting.render.flush(|raw| {
        output = Some(serde_json::from_str::<Value>(&raw).unwrap());
        true
    }));
    output.unwrap()
}
#[test]
fn existing_owner_validator_binds_actual_sources_without_metadata_creating_identity() {
    let mut host = host();
    let state = frame(&mut host);
    assert_eq!(state["enabled"], true);
    assert_eq!(state["entityLights"][0]["key"], "42");
    assert!(host.bind_lighting_snapshot(&raw("0", "Wrong")).is_err());
    assert!(host.lighting.is_bound());
    // Invalid input did not partially replace the committed scene.
    assert!(
        host.lighting
            .render
            .flush(|_| panic!("Invalid owner dirtied retry"))
    );
}
#[test]
fn original_personal_reset_clears_actual_sources_and_old_queued_retry() {
    let mut host = host();
    let environment = host.lighting.render_environment().clone();
    host.lighting
        .render
        .produce(
            &environment,
            true,
            Some((10, 20)),
            |_| true,
            &NativeLightingMotion::default(),
            &[],
        )
        .unwrap();
    assert!(!host.lighting.render.flush(|_| false));
    let generation = host.lighting.render.generation();
    host.reset_personal();
    assert!(host.lighting.render.generation() > generation);
    assert!(!host.lighting.is_bound());
    assert!(
        host.lighting
            .render
            .flush(|_| panic!("Old lighting retry survived original host reset"))
    );
    assert_eq!(frame(&mut host)["enabled"], false);
}
#[test]
fn map_metadata_retains_time_but_cannot_promote_unbound_destination_sources() {
    let mut host = host();
    assert_eq!(frame(&mut host)["enabled"], true);
    host.accept_lighting_packet(
        Screen::InGame,
        &json!({
        "type":"packet","packet":"MapChanged","payload":{"fileName":"other","lights":3}})
        .to_string(),
    )
    .unwrap();
    assert_eq!(frame(&mut host)["enabled"], false);
    host.lighting.clear_scene();
    assert_eq!(
        host.lighting.environment().time_of_day_light_setting,
        Some(4)
    );
    assert!(!host.lighting.is_bound());
    assert_eq!(frame(&mut host)["enabled"], false);
}
