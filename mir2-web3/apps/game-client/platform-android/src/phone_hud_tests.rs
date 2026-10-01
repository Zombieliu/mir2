use super::*;

#[test]
fn phone_chat_leaves_thumb_zones_clear_and_belt_has_real_tap_targets() {
    let layout = phone_layout(Vec2::new(851.0, 393.0), Vec4::ZERO, 0.0, 0.51, false, 4);
    assert_eq!(layout.chat.width(), 480.0);
    assert!(layout.chat.min.x > 24.0 + 63.0);
    assert!(layout.chat.max.x < 851.0 - 20.0 - 116.0);
    assert_eq!(layout.belt.width(), 48.0 * 6.0);
    assert_eq!(layout.belt.height(), 48.0);
    assert!(layout.belt.max.y < layout.chat.min.y);
}

#[test]
fn keyboard_and_safe_insets_are_counted_once() {
    let layout = phone_layout(
        Vec2::new(851.0, 393.0),
        Vec4::new(18.0, 8.0, 22.0, 12.0),
        170.0,
        0.51,
        true,
        4,
    );
    assert_eq!(layout.chat.max.y, 393.0 - 170.0 - 12.0);
    assert!(layout.chat.min.x >= 18.0 + 16.0);
    assert!(layout.chat.max.x <= 851.0 - 22.0 - 16.0);
    assert!(layout.chat.min.y >= 8.0);
}

#[test]
fn narrow_landscape_still_has_a_chat_entry_and_three_touch_controls() {
    for (width, height) in [
        (568.0, 262.0),
        (640.0, 320.0),
        (851.0, 393.0),
        (1200.0, 800.0),
    ] {
        let layout = phone_layout(Vec2::new(width, height), Vec4::ZERO, 0.0, 0.4, false, 4);
        assert!(layout.chat.width() - TAP * 3.0 - 8.0 >= TAP);
        assert!(layout.chat.min.x >= 0.0);
        assert!(layout.chat.max.x <= width);
        assert!(layout.chat.max.y <= height);
    }
}

#[test]
fn compact_thumb_row_and_chat_do_not_overlap() {
    for width in [480.0, 568.0, 585.0, 640.0] {
        let layout = phone_layout(Vec2::new(width, 270.0), Vec4::ZERO, 0.0, 0.35, false, 4);
        let thumbs = crate::mobile_ui::thumb_footprints(270.0);
        let pad = Rect::from_corners(
            Vec2::new(width - thumbs.y + 16.0, 270.0 - 20.0 - 48.0),
            Vec2::new(width - 20.0, 270.0 - 20.0),
        );
        assert!(layout.chat.intersect(pad).is_empty(), "width={width}");
        assert!(layout.chat.width() - TAP * 3.0 - 8.0 >= TAP);
    }
}

fn app() -> App {
    let mut app = App::new();
    let mut player = NativePlayerUiState::default();
    player.core.screen = mir2_ui_core::state::UiScreen::InGame;
    app.insert_resource(UiScale(0.5))
        .init_resource::<AndroidShellState>()
        .insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        })
        .insert_resource(player)
        .init_resource::<UiReadModel>()
        .init_resource::<CrystalChatState>()
        .init_resource::<CrystalBeltPresentation>()
        .init_resource::<crate::shared_shell::HostState>()
        .init_resource::<CrystalChatPointerBounds>()
        .add_systems(Startup, spawn)
        .add_systems(
            PostUpdate,
            (decorate_belt, fit_phone_hud, use_android_system_fonts).chain(),
        );
    let mut window = Window::default();
    window.resolution.set_scale_factor_override(Some(1.0));
    window.resolution.set(851.0, 393.0);
    app.world_mut().spawn(window);
    app
}

#[test]
fn shared_controls_survive_adaptation_and_desktop_frame_is_hidden() {
    let mut app = app();
    let desktop = app.world_mut().spawn(Node::default()).id();
    let target = app
        .world_mut()
        .spawn((
            Node::default(),
            CrystalHudBeltHitTarget { slot: 2 },
            CrystalHudAction::BeltUse(2),
            Button,
        ))
        .id();
    let belt = app
        .world_mut()
        .spawn((Node::default(), CrystalHudBeltLayer))
        .id();
    app.world_mut().entity_mut(belt).add_child(target);
    let root = app
        .world_mut()
        .spawn((Node::default(), CrystalHudRoot))
        .id();
    app.world_mut()
        .entity_mut(root)
        .add_children(&[desktop, belt]);
    let frame = app
        .world_mut()
        .spawn((
            Node::default(),
            CrystalChatElement,
            CrystalChatBackdrop,
            ImageNode::default(),
        ))
        .id();
    app.world_mut().spawn((Node::default(), CrystalChatRoot));
    app.update();
    assert_eq!(
        app.world().get::<Node>(desktop).unwrap().display,
        Display::None
    );
    assert_eq!(
        app.world().get::<CrystalHudAction>(target),
        Some(&CrystalHudAction::BeltUse(2))
    );
    assert_eq!(
        val_px(app.world().get::<Node>(target).unwrap().width) * 0.5,
        48.0
    );
    assert_eq!(
        app.world().get::<ImageNode>(frame).unwrap().color,
        Color::NONE
    );
    assert!(app.world().get::<BackgroundColor>(frame).unwrap().0.alpha() < 1.0);
    assert!(app
        .world()
        .resource::<CrystalChatPointerBounds>()
        .0
        .is_some());
}

#[test]
fn nonmodal_views_keep_phone_status_and_chat_entry_without_desktop_display_resource() {
    use mir2_ui_core::state::UiPanel;
    for panel in [
        UiPanel::Inventory,
        UiPanel::Character,
        UiPanel::Skill,
        UiPanel::Options,
        UiPanel::Menu,
        UiPanel::QuestLog,
    ] {
        let mut app = app();
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .core
            .panel = panel;
        app.update();
        assert!(app
            .world()
            .get_resource::<mir2_client_bevy::native_display::NativeDisplaySettings>()
            .is_none());
        let world = app.world_mut();
        for display in [
            world
                .query_filtered::<&Node, With<PhoneStatus>>()
                .single(world)
                .unwrap()
                .display,
            world
                .query_filtered::<&Node, With<ChatEntry>>()
                .single(world)
                .unwrap()
                .display,
        ] {
            assert_eq!(
                display,
                Display::Flex,
                "{panel:?} hid an ordinary phone affordance"
            );
        }
    }
}

#[test]
fn shared_rebuild_keeps_phone_nodes_and_does_not_change_chat_settings() {
    let mut app = app();
    app.world_mut().spawn((Node::default(), CrystalChatRoot));
    let settings = app
        .world()
        .resource::<NativePlayerUiState>()
        .core
        .chat_settings;
    for _ in 0..3 {
        let frame = app
            .world_mut()
            .spawn((
                Node::default(),
                CrystalChatElement,
                CrystalChatBackdrop,
                ImageNode::default(),
            ))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<ImageNode>(frame).unwrap().color,
            Color::NONE
        );
        assert_eq!(
            app.world()
                .resource::<NativePlayerUiState>()
                .core
                .chat_settings,
            settings
        );
        app.world_mut().despawn(frame);
    }
}

#[test]
fn chat_settings_descendants_are_not_reflowed_or_hidden() {
    let mut app = app();
    let root = app
        .world_mut()
        .spawn((Node::default(), CrystalChatRoot))
        .id();
    let modal = app
        .world_mut()
        .spawn((
            Node::default(),
            CrystalChatSettingsModal,
            CrystalChatElement,
        ))
        .id();
    let button = app
        .world_mut()
        .spawn((
            Node {
                left: px(24),
                top: px(32),
                width: px(18),
                height: px(20),
                ..default()
            },
            CrystalChatElement,
            CrystalChatAction::SettingsApply,
        ))
        .id();
    app.world_mut().entity_mut(modal).add_child(button);
    app.world_mut().entity_mut(root).add_child(modal);
    app.update();
    let node = app.world().get::<Node>(button).unwrap();
    assert_eq!(node.display, Display::Flex);
    assert_eq!(node.left, px(24));
    assert_eq!(node.width, px(18));
}

#[test]
fn empty_chat_does_not_reserve_blank_desktop_history_space() {
    let empty = phone_layout(Vec2::new(851.0, 393.0), Vec4::ZERO, 0.0, 0.51, false, 0);
    let populated = phone_layout(Vec2::new(851.0, 393.0), Vec4::ZERO, 0.0, 0.51, false, 4);
    assert_eq!(empty.chat.height(), TAP + 8.0);
    assert_eq!(empty.chat.max.y, populated.chat.max.y);
    assert!(empty.belt.min.y > populated.belt.min.y);
}

#[test]
fn android_family_resolution_preserves_explicit_fonts_and_shared_default() {
    assert_eq!(
        crystal_text_font(12.0).font,
        FontSource::Family("Arial".into())
    );
    let mut app = App::new();
    let android = app.world_mut().spawn(crystal_text_font(12.0)).id();
    let explicit = app
        .world_mut()
        .spawn(TextFont {
            font: FontSource::Monospace,
            ..default()
        })
        .id();
    app.add_systems(Update, use_android_system_fonts);
    app.update();
    assert_eq!(
        app.world().get::<TextFont>(android).unwrap().font,
        FontSource::SystemUi
    );
    assert_eq!(
        app.world().get::<TextFont>(explicit).unwrap().font,
        FontSource::Monospace
    );
    assert_eq!(
        crystal_text_font(12.0).font,
        FontSource::Family("Arial".into())
    );
}

#[test]
fn focused_chat_retains_all_source_channel_trade_and_size_actions() {
    assert_eq!(CrystalChatFilter::all_variants().len(), 7);
    assert_eq!(phone_filter_index(CrystalChatAction::FilterGuild), Some(6));
    assert_eq!(phone_filter_index(CrystalChatAction::TradeRequest), Some(7));
    assert_eq!(phone_filter_index(CrystalChatAction::Resize), Some(8));
    assert_eq!(phone_filter_index(CrystalChatAction::SettingsApply), None);
}

#[test]
fn phone_chat_control_keeps_semantic_button_with_its_own_readable_label() {
    let mut app = app();
    let root = app
        .world_mut()
        .spawn((Node::default(), CrystalChatRoot))
        .id();
    let button = app
        .world_mut()
        .spawn((
            Node::default(),
            ImageNode::default(),
            Button,
            CrystalChatElement,
            CrystalChatAction::Settings,
        ))
        .id();
    app.world_mut().entity_mut(root).add_child(button);
    app.update();
    assert!(app.world().get::<ImageNode>(button).is_none());
    assert!(app.world().get::<Button>(button).is_some());
    assert_eq!(app.world().get::<ZIndex>(button), Some(&ZIndex(1)));
    assert_eq!(
        app.world().get::<CrystalChatAction>(button),
        Some(&CrystalChatAction::Settings)
    );
    assert_eq!(app.world().get::<Text>(button).unwrap().0, "Set");
    assert_eq!(
        app.world().get::<TextFont>(button).unwrap().font,
        FontSource::SystemUi
    );
    assert_eq!(
        app.world().get::<Node>(button).unwrap().display,
        Display::Flex
    );
}

#[test]
fn modal_and_login_hide_phone_chrome_without_mutating_player_state() {
    let mut app = app();
    app.update();
    app.world_mut().resource_mut::<NativeShellModel>().screen = NativeShellScreen::Login;
    app.update();
    for node in app
        .world_mut()
        .query_filtered::<&Node, With<PhoneStatus>>()
        .iter(app.world())
    {
        assert_eq!(node.display, Display::None);
    }
    assert_eq!(
        app.world().resource::<NativePlayerUiState>().core.screen,
        mir2_ui_core::state::UiScreen::InGame
    );
}
