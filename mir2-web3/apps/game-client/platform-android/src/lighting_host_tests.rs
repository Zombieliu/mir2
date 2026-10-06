//! Headless tests of the actual Android host reducer/registration. These are
//! not JNI, GPU, online authentication, or physical-device acceptance.
use super::*;

fn app() -> App {
    INBOX.lock().unwrap().clear();
    OUTBOX.lock().unwrap().clear();
    let mut app = App::new();
    app.add_plugins(mir2_bevy_runtime::Mir2NativeSessionBoundaryPlugin);
    mir2_bevy_runtime::native_ingest::install_native_ingestion(&mut app);
    #[cfg(feature = "ui-preview")]
    app.init_resource::<crate::ui_preview::PreviewRequest>();
    app.insert_resource(NativeShellModel {
        screen: Screen::StartingGame,
        ..default()
    })
    .insert_resource(HostState {
        phase: "STARTING".into(),
        ..default()
    })
    .init_resource::<NativeUiIntentQueue>()
    .add_systems(PreUpdate, receive);
    app
}
fn host_packet(packet: &str, payload: Value) -> Value {
    json!({"type":"gatewayGameplayPacket","envelope":
        json!({"type":"packet","packet":packet,"payload":payload}).to_string()})
}
fn host_world(map: &str) -> Value {
    let raw = json!({"playerObjectId":42,"mapFileName":map,
        "entities":[{"kind":"selfPlayer","objectId":42,"name":"Fixture","x":10,"y":20}]});
    json!({"phase":"IN_GAME","world":{"playerName":"Fixture","mapFileName":map,"x":10,"y":20},
        "worldSnapshot":raw.to_string()})
}

#[test]
fn lighting_host_requires_both_authenticated_phase_and_shared_world_screen() {
    let raw = json!({"type":"packet","packet":"TimeOfDay","payload":{"lights":4}}).to_string();
    let mut host = HostState::default();
    for phase in ["READY", "LOGIN", "CHARACTERS", "DISCONNECTED", "CONNECTING"] {
        host.phase = phase.into();
        assert!(
            !host
                .accept_lighting_packet(Screen::StartingGame, &raw)
                .unwrap()
        );
    }
    host.phase = "STARTING".into();
    for screen in [
        Screen::Login,
        Screen::CharacterSelect,
        Screen::ConnectionLost,
    ] {
        assert!(!host.accept_lighting_packet(screen, &raw).unwrap());
    }
    assert!(
        host.accept_lighting_packet(Screen::StartingGame, &raw)
            .unwrap()
    );
    assert!(!host.lighting.is_bound());
    assert!(host.lighting.flush(|_| panic!("Metadata bypassed owner")));
    assert!(host.world.is_none() && host.pending_render_request.is_none());
}

#[test]
fn lighting_host_real_receive_registers_metadata_snapshot_and_original_render_barrier() {
    let mut app = app();
    INBOX.lock().unwrap().extend([
        host_packet("TimeOfDay", json!({"lights":4})),
        host_packet(
            "MapInformation",
            json!({"fileName":"0","lights":3,"mapDarkLight":2}),
        ),
    ]);
    app.update();
    let host = app.world().resource::<HostState>();
    assert!(!host.lighting.is_bound());
    assert_eq!(
        host.lighting.environment().time_of_day_light_setting,
        Some(4)
    );
    assert_eq!(host.lighting.environment().map_light_setting, Some(3));
    assert!(host.pending_render_request.is_none());
    INBOX.lock().unwrap().push_back(host_world("0"));
    app.update();
    let host = app.world().resource::<HostState>();
    assert!(host.lighting.is_bound());
    assert_eq!(host.lighting.environment().map_dark_light, 2);
    assert!(host.pending_render_request.is_some());
    assert_eq!(
        app.world().resource::<NativeShellModel>().screen,
        Screen::StartingGame
    );
    assert!(OUTBOX.lock().unwrap().is_empty());
}

#[test]
fn lighting_host_map_transition_retains_connection_time_not_old_scene() {
    let mut app = app();
    INBOX.lock().unwrap().extend([
        host_world("0"),
        host_packet("TimeOfDay", json!({"lights":4})),
        host_packet(
            "NewMapInfo",
            json!({"fileName":"0","lights":3,"mapDarkLight":2}),
        ),
    ]);
    app.update();
    INBOX
        .lock()
        .unwrap()
        .push_back(json!({"phase":"STARTING","message":"Destination scene pending"}));
    app.update();
    let host = app.world().resource::<HostState>();
    assert!(!host.lighting.is_bound());
    assert_eq!(
        host.lighting.environment().time_of_day_light_setting,
        Some(4)
    );
    assert_eq!(host.lighting.environment().map_light_setting, None);
    assert_eq!(host.lighting.environment().map_dark_light, 0);
    INBOX.lock().unwrap().push_back(host_world("1"));
    app.update();
    let host = app.world().resource::<HostState>();
    assert!(host.lighting.is_bound());
    assert_eq!(
        host.lighting.environment().current_map_file_name.as_deref(),
        Some("1")
    );
    assert_eq!(
        host.lighting.environment().time_of_day_light_setting,
        Some(4)
    );
}

#[test]
fn lighting_host_malformed_environment_is_terminal_and_reset_clears_all_state() {
    let mut app = app();
    INBOX.lock().unwrap().extend([
        host_world("0"),
        host_packet("TimeOfDay", json!({"lights":4})),
        host_packet("MapInformation", json!({"fileName":"bad\nmap","lights":3})),
    ]);
    app.update();
    let host = app.world().resource::<HostState>();
    assert_eq!(host.phase, "DISCONNECTED");
    assert!(!host.lighting.is_bound());
    assert_eq!(host.lighting.environment(), &Default::default());
    assert!(host.world.is_none() && host.pending_render_request.is_none());
    assert_eq!(
        app.world().resource::<NativeShellModel>().screen,
        Screen::ConnectionLost
    );
    let mut out = OUTBOX.lock().unwrap();
    let command: Value = serde_json::from_str(&out.pop_front().unwrap()).unwrap();
    assert_eq!(command["type"], "disconnect");
    assert!(out.is_empty());
}

#[test]
fn lighting_host_render_failure_and_disconnect_retire_unpublished_metadata() {
    let mut host = HostState {
        phase: "STARTING".into(),
        pending_render_request: Some(7),
        ..default()
    };
    let raw = host_packet("TimeOfDay", json!({"lights":4}))["envelope"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        host.accept_lighting_packet(Screen::StartingGame, &raw)
            .unwrap()
    );
    let mut shell = NativeShellModel {
        screen: Screen::StartingGame,
        ..default()
    };
    assert!(fail_current_render_load(
        &mut host,
        &mut shell,
        7,
        "Offline fixture failure"
    ));
    assert_eq!(host.lighting.environment(), &Default::default());
    assert!(!host.lighting.is_bound());
    assert!(host.lighting.flush(|_| panic!("Retired metadata")));
}

#[test]
fn lighting_snapshot_binding_uses_existing_validated_player_not_raw_metadata_identity() {
    let raw = host_world("0")["worldSnapshot"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut host = HostState::default();
    assert!(host.bind_lighting_snapshot(&raw).is_err());
    host.player.snapshot(&raw).unwrap();
    host.bind_lighting_snapshot(&raw).unwrap();
    assert!(host.lighting.is_bound());
    let mut wrong: Value = serde_json::from_str(&raw).unwrap();
    wrong["playerObjectId"] = json!(99);
    wrong["entities"][0]["objectId"] = json!(99);
    assert!(host.bind_lighting_snapshot(&wrong.to_string()).is_err());
    assert!(host.lighting.is_bound());
}
