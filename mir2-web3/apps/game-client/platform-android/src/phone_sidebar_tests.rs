use super::*;

#[test]
fn sidebar_reflows_the_six_original_belt_actions_and_counts_without_changing_preferences() {
    let mut app = App::new();
    let mut player = NativePlayerUiState::default();
    player.core.screen = mir2_ui_core::state::UiScreen::InGame;
    player.core.panel = mir2_ui_core::state::UiPanel::Inventory;
    app.insert_resource(player)
        .insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        })
        .insert_resource(UiScale(0.51))
        .init_resource::<AndroidShellState>()
        .init_resource::<UiReadModel>()
        .init_resource::<CrystalChatState>()
        .init_resource::<crate::shared_shell::HostState>()
        .init_resource::<CrystalChatPointerBounds>()
        .insert_resource(CrystalBeltPresentation {
            visible: true,
            vertical: true,
        })
        .add_systems(Startup, spawn)
        .add_systems(Update, fit_phone_hud);
    let mut window = Window::default();
    window.resolution.set_scale_factor_override(Some(1.0));
    window.resolution.set(851.0, 393.0);
    app.world_mut().spawn(window);
    let layer = app
        .world_mut()
        .spawn((CrystalHudBeltLayer, Node::default()))
        .id();
    let mut targets = Vec::new();
    let mut counts = Vec::new();
    for slot in 0..6 {
        let target = app
            .world_mut()
            .spawn((
                Node::default(),
                CrystalHudBeltHitTarget { slot },
                CrystalHudAction::BeltUse(slot),
                Button,
            ))
            .id();
        let count = app
            .world_mut()
            .spawn((Node::default(), CrystalHudBeltItem { slot }, Text::new("3")))
            .id();
        app.world_mut()
            .entity_mut(layer)
            .add_children(&[target, count]);
        targets.push(target);
        counts.push(count);
    }
    let hud = app
        .world_mut()
        .spawn((CrystalHudRoot, Node::default()))
        .id();
    app.world_mut().entity_mut(hud).add_child(layer);
    app.world_mut().spawn((CrystalChatRoot, Node::default()));
    app.update();
    let belt_node = app.world().get::<Node>(layer).unwrap();
    assert_eq!(belt_node.display, Display::Flex);
    assert_eq!(
        Vec2::new(val_px(belt_node.width), val_px(belt_node.height)) * 0.51,
        Vec2::new(144.0, 96.0)
    );
    let mut rectangles = Vec::new();
    for slot in 0..6 {
        let node = app.world().get::<Node>(targets[slot as usize]).unwrap();
        let min = Vec2::new(val_px(node.left), val_px(node.top)) * 0.51;
        let size = Vec2::new(val_px(node.width), val_px(node.height)) * 0.51;
        assert_eq!(min, phone_belt_slot(slot, true));
        assert_eq!(size, Vec2::splat(48.0));
        assert_eq!(
            app.world().get::<CrystalHudAction>(targets[slot as usize]),
            Some(&CrystalHudAction::BeltUse(slot))
        );
        let count = app.world().get::<Node>(counts[slot as usize]).unwrap();
        assert!((val_px(count.top) * 0.51 - min.y - 34.0).abs() < 0.001);
        rectangles.push(Rect::from_corners(min, min + size));
    }
    for (index, a) in rectangles.iter().enumerate() {
        for b in rectangles.iter().skip(index + 1) {
            assert!(a.intersect(*b).is_empty());
        }
    }
    assert!(app.world().resource::<CrystalBeltPresentation>().vertical);
    assert!(app.world().resource::<CrystalBeltPresentation>().visible);
    let status = app
        .world_mut()
        .query_filtered::<&Node, With<PhoneStatus>>()
        .single(app.world())
        .unwrap();
    assert_eq!(val_px(status.height) * 0.51, 76.0);
    assert_eq!(status.display, Display::Flex);
    let bounds = app
        .world()
        .resource::<CrystalChatPointerBounds>()
        .0
        .unwrap();
    assert_eq!(
        bounds,
        crate::phone_panels::panel_sidebar(Vec2::new(851.0, 393.0), Vec4::ZERO, 0.0, true)
            .unwrap()
            .chat
    );

    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .core
        .panel = mir2_ui_core::state::UiPanel::None;
    app.update();
    let node = app.world().get::<Node>(layer).unwrap();
    assert_eq!(val_px(node.width) * 0.51, 288.0);
    assert_eq!(val_px(node.height) * 0.51, 48.0);
    assert!(app.world().resource::<CrystalBeltPresentation>().vertical);
}
