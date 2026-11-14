use super::*;
fn id(generation: u64) -> Identity {
    Identity {
        session_epoch: 1,
        hero_generation: generation,
        object_id: 12,
    }
}
fn buff(identity: Identity, owner: u32, now: Instant) -> Event {
    Event::Buff{identity,event:BuffEvent::parse("AddBuff",&serde_json::json!({"objectId":owner,"buffType":24,"remainingMs":10000,"infinite":false,"paused":false,"stats":[],"values":[]}),now).unwrap()}
}
#[test]
fn exact_hero_identity_filters_player_old_hero_and_refresh_retains_buffs() {
    let now = Instant::now();
    let mut ui = HeroBuffHud::default();
    ui.observe(&Event::Information(id(1)), now);
    ui.observe(&buff(id(1), 11, now), now);
    assert!(ui.rows.buffs.is_empty());
    ui.observe(&buff(id(1), 12, now), now);
    assert_eq!(ui.rows.buffs.len(), 1);
    ui.observe(&Event::Information(id(1)), now);
    assert_eq!(ui.rows.buffs.len(), 1);
    ui.observe(&Event::Information(id(2)), now);
    ui.observe(&buff(id(1), 12, now), now);
    assert!(ui.rows.buffs.is_empty());
    ui.observe(&buff(id(2), 12, now), now);
    assert_eq!(ui.rows.buffs.len(), 1);
}
#[test]
fn ordinary_recall_disposes_buffs_and_dead_state_keeps_page() {
    let now = Instant::now();
    let mut ui = HeroBuffHud::default();
    ui.observe(&Event::Information(id(1)), now);
    ui.observe(&buff(id(1), 12, now), now);
    ui.observe(&Event::SpawnState(3), now);
    assert!(ui.visible);
    assert_eq!(ui.rows.buffs.len(), 1);
    ui.observe(&Event::SpawnState(1), now);
    assert!(!ui.visible);
    assert!(ui.rows.buffs.is_empty());
    ui.observe(&buff(id(1), 12, now), now);
    assert!(ui.rows.buffs.is_empty());
    ui.observe(&Event::SpawnState(2), now);
    assert!(!ui.visible);
    ui.observe(&Event::Information(id(1)), now);
    assert!(ui.visible);
    assert!(ui.rows.buffs.is_empty());
}
#[test]
fn hero_buff_render_is_separate_and_survives_source_camera_mode() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .init_asset::<Image>()
        .init_resource::<NativePlayerUiState>()
        .init_resource::<NativeShellModel>()
        .add_systems(Update, render);
    app.world_mut().spawn((OverlayRoot, Node::default()));
    app.world_mut().resource_mut::<NativeShellModel>().screen = NativeShellScreen::InGame;
    let now = Instant::now();
    {
        let mut ui = app.world_mut().resource_mut::<NativePlayerUiState>();
        ui.local_keys.camera_hidden = true;
        ui.hero_buffs.observe(&Event::Information(id(1)), now);
        ui.hero_buffs.observe(&buff(id(1), 12, now), now);
        ui.core.options.expanded_buff_window = false;
        assert!(ui.core.options.expanded_hero_buff_window);
        assert_eq!(rect(&ui).top, 80.);
    }
    app.update();
    assert_eq!(
        app.world_mut()
            .query::<&HeroBuffRoot>()
            .iter(app.world())
            .count(),
        1
    );
    let paths = app
        .world_mut()
        .query::<&ImageNode>()
        .iter(app.world())
        .filter_map(|n| n.image.path().map(ToString::to_string))
        .collect::<Vec<_>>();
    assert!(paths.iter().any(|p| p.ends_with("BuffIcon/30.png")));
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .hero_buffs
        .observe(&Event::SpawnState(0), now);
    app.update();
    assert_eq!(
        app.world_mut()
            .query::<&HeroBuffRoot>()
            .iter(app.world())
            .count(),
        0
    );
}
#[test]
fn ordinary_hero_spawn_state_updates_host_and_rejects_invalid_state() {
    let mut model = crate::hero_model::HeroModel::default();
    assert!(model.apply_packet("UpdateHeroSpawnState", &serde_json::json!({"state":2})));
    assert!(model.spawned);
    assert!(!model.apply_packet("UpdateHeroSpawnState", &serde_json::json!({"state":4})));
    assert!(model.spawned);
    assert!(model.apply_packet("UpdateHeroSpawnState", &serde_json::json!({"state":1})));
    assert!(!model.spawned);
}
