use super::*;
use crate::inventory::{CrystalItemTooltipSourceModel, CrystalUserItemModel};
#[test]
fn real_source_frames_and_threshold_boundaries() {
    assert_eq!(buff_icon("MagicShield"), 30);
    assert_eq!(buff_icon("TemporalFlux"), 261);
    assert_eq!(frame_size("Prguse2", 7), Some(Vec2::new(16., 15.)));
    let mut item: ItemModel = serde_json::from_value(
        serde_json::json!({"key":"sword","name":"Sword","quantity":1,"slot":0,"container":2}),
    )
    .unwrap();
    let mut source = CrystalItemTooltipSourceModel::default();
    source.info.item_type = 1;
    source.user_item = Some(CrystalUserItemModel {
        current_dura: 51,
        max_dura: 100,
        ..Default::default()
    });
    item.tooltip_source = Some(source);
    assert_eq!(durability_frame(0, &item), Some(2125));
    for (dura, frame) in [(50, Some(2126)), (20, Some(2127)), (0, None)] {
        item.tooltip_source
            .as_mut()
            .unwrap()
            .user_item
            .as_mut()
            .unwrap()
            .current_dura = dura;
        assert_eq!(durability_frame(0, &item), frame);
    }
    item.tooltip_source.as_mut().unwrap().info.item_type = 8;
    item.tooltip_source.as_mut().unwrap().info.stack_size = 5000;
    item.quantity = 1000;
    assert_eq!(durability_frame(9, &item), Some(2136));
    item.tooltip_source = None;
    assert_eq!(durability_frame(9, &item), None);
}
fn add(owner: u32, kind: &str, now: Instant) -> BuffEvent {
    BuffEvent::parse("AddBuff",&serde_json::json!({"objectId":owner,"buffType":kind,"remainingMs":10000,"infinite":false,"paused":false,"stats":[],"values":[]}),now).unwrap()
}
#[test]
fn owner_pause_refresh_remove_and_identity_are_authoritative() {
    let now = Instant::now();
    let mut ui = StatusHud::default();
    ui.observe(Some(1), &add(2, "Fury", now), now);
    assert!(ui.buffs.is_empty());
    ui.observe(Some(1), &add(1, "Fury", now), now);
    ui.observe(Some(1), &add(1, "Rage", now), now);
    ui.observe(Some(1), &add(1, "Fury", now), now);
    assert_eq!(ui.buffs.len(), 2);
    assert_eq!(ui.buffs[0].kind, "Fury");
    let later = now + std::time::Duration::from_secs(3);
    ui.observe(
        Some(1),
        &BuffEvent::Pause {
            owner: 1,
            kind: "Fury".into(),
            paused: true,
            received: later,
        },
        later,
    );
    assert_eq!(
        ui.buffs[0].remaining(later + std::time::Duration::from_secs(20)),
        7000
    );
    ui.observe(
        Some(1),
        &BuffEvent::Remove {
            owner: 1,
            kind: "Rage".into(),
        },
        later,
    );
    assert_eq!(ui.buffs.len(), 1);
    ui.observe(Some(2), &add(2, "Haste", later), later);
    assert_eq!(ui.buffs.len(), 1);
    assert_eq!(ui.buffs[0].kind, "Haste");
}
#[test]
fn malformed_buff_does_not_mutate_and_blink_uses_source_rounding() {
    let now = Instant::now();
    assert!(BuffEvent::parse(
        "AddBuff",
        &serde_json::json!({"objectId":1,"buffType":"Fury"}),
        now
    )
    .is_none());
    let BuffEvent::Add { mut buff, .. } = add(1, "Fury", now) else {
        panic!()
    };
    assert!(!buff.blink_hidden(now));
    assert!(buff.blink_hidden(now + std::time::Duration::from_millis(5000)));
    buff.paused = true;
    assert!(!buff.blink_hidden(now + std::time::Duration::from_secs(30)));
    assert_eq!(buff_geometry(10, true).1, Vec2::new(250., 34.));
    assert_eq!(buff_geometry(21, true).0, 30);
}

#[test]
fn camera_mode_hides_and_restores_real_dura_and_buff_images() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .init_asset::<Image>()
        .init_resource::<NativePlayerUiState>()
        .init_resource::<InventoryModel>()
        .init_resource::<NativeShellModel>()
        .add_systems(Update, render);
    app.world_mut().spawn((OverlayRoot, Node::default()));
    app.world_mut().resource_mut::<NativeShellModel>().screen = NativeShellScreen::InGame;
    let now = Instant::now();
    {
        let mut ui = app.world_mut().resource_mut::<NativePlayerUiState>();
        ui.core.options.dura_view = true;
        ui.status_hud
            .observe(Some(1), &add(1, "MagicShield", now), now);
    }
    app.update();
    assert_eq!(
        app.world_mut()
            .query::<&StatusRoot>()
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
    assert!(paths.iter().any(|p| p.ends_with("Prguse/2105.png")));
    assert!(paths.iter().any(|p| p.ends_with("BuffIcon/30.png")));
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .local_keys
        .camera_hidden = true;
    app.update();
    assert_eq!(
        app.world_mut()
            .query::<&StatusRoot>()
            .iter(app.world())
            .count(),
        0
    );
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .local_keys
        .camera_hidden = false;
    app.update();
    assert_eq!(
        app.world_mut()
            .query::<&StatusRoot>()
            .iter(app.world())
            .count(),
        1
    );
    assert!(
        app.world()
            .resource::<NativePlayerUiState>()
            .core
            .options
            .dura_view
    );
}
#[test]
fn hints_use_source_special_stats_percent_and_elapsed_time() {
    let now = Instant::now();
    let BuffEvent::Add { mut buff, .. } = add(1, "MagicBooster", now) else {
        panic!()
    };
    buff.stats = vec![
        (6, "Min MC".into(), 2),
        (7, "Max MC".into(), 6),
        (127, "Mana Penalty Percent".into(), 20),
    ];
    let s = buff_hint(&buff, now);
    assert!(s.contains("Increases MC by: 2-6."));
    assert!(s.contains("Increases consumption by 20%."));
    assert!(s.contains("10s"));
    assert!(!s.contains("Caster:"));
    buff.kind = "Rage".into();
    assert!(combined_hint(&[buff]).contains("20%."));
}

#[test]
fn ordinary_numeric_buff_discriminant_resolves_actual_icon() {
    let now = Instant::now();
    let p = serde_json::json!({"objectId":42,"buffType":24,"remainingMs":10000,"infinite":false,"paused":false,"visible":false,"stats":[{"stat":6,"label":"MinMC","value":2}],"values":[]});
    let BuffEvent::Add { buff, .. } = BuffEvent::parse("AddBuff", &p, now).unwrap() else {
        panic!()
    };
    assert_eq!(buff.kind, "MagicShield");
    assert_eq!(buff_icon(&buff.kind), 30);
    let mut state = StatusHud::default();
    state.observe(Some(42), &BuffEvent::Add { owner: 42, buff }, now);
    assert_eq!(state.buffs.len(), 1);
}

#[test]
fn paused_packet_uses_receipt_time_even_if_ui_drain_is_late() {
    let now = Instant::now();
    let mut ui = StatusHud::default();
    ui.observe(Some(1), &add(1, "Fury", now), now);
    let received = now + std::time::Duration::from_secs(2);
    let event = BuffEvent::parse(
        "PauseBuff",
        &serde_json::json!({"objectId":1,"buffType":5,"paused":true}),
        received,
    )
    .unwrap();
    ui.observe(Some(1), &event, now + std::time::Duration::from_secs(9));
    assert_eq!(
        ui.buffs[0].remaining(now + std::time::Duration::from_secs(90)),
        8000
    );
}

#[test]
fn camera_restore_shows_dura_without_changing_saved_source_preference() {
    let mut state = NativePlayerUiState::default();
    assert!(!state.core.options.dura_view);
    assert!(!state.status_hud.dura_visible(state.core.options.dura_view));
    state.status_hud.restore_camera();
    assert!(state.status_hud.dura_visible(state.core.options.dura_view));
    assert!(!state.core.options.dura_view);
    assert!(button_at(&state, Vec2::new(1022.9, 160.)).is_some());
    assert!(button_at(&state, Vec2::new(1023., 160.)).is_none());
}

#[test]
fn collapsed_buff_count_uses_centered_original_bold_font() {
    let mut app = App::new();
    app.add_systems(Startup, |mut c: Commands| {
        c.spawn(Node::default())
            .with_children(|p| count_label(p, 12, CrystalRect::new(0., 0., 43., 20.)));
    });
    app.update();
    let (font, text) = app
        .world_mut()
        .query::<(&TextFont, &Text)>()
        .single(app.world())
        .unwrap();
    assert_eq!(text.0, "12");
    assert_eq!(font.font, FontSource::Family("Arial".into()));
    assert_eq!(font.weight, bevy::text::FontWeight::BOLD);
    assert_eq!(font.font_size, FontSize::Px(40. / 3.));
}
